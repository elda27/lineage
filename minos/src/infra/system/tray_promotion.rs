//! トレイアイコンを通知領域へ「昇格」させる。
//!
//! Windows 11 は初めて見るトレイアイコンをオーバーフロー（`^`）の中に隠す。
//! minos は Alt+Space で呼ぶアプリなので致命的ではないが、常駐していることが
//! 分からないのは困る。そこで初回だけ、見える側に出す。
//!
//! `Shell_NotifyIcon` に昇格を要求するオプションは無く（Windows 7 で塞がれた）、
//! Windows 11 にも代替 API は無い。残る手段は Windows 自身が持つ表示設定
//! `HKCU\Control Panel\NotifyIconSettings\<hash>\IsPromoted` を直接書くことだけ。
//! **非公開の仕様**なので、失敗しても警告を出すだけで起動は続ける。
//!
//! 利用者が明示的に「隠す」を選んだ場合はその意思が同じ値に入る。
//! そのため書くのは値が**まだ無いとき**だけ。一度でも設定されていれば触らない。
//!
//! サブキー名は exe のパスと `Shell_NotifyIcon` の uID から導かれるハッシュで、
//! 算出方法は非公開。よって走査して `ExecutablePath` が自分と一致するものを探す。
//! uID は tray-icon クレートのプロセス内カウンタなので、トレイアイコンを1つだけ
//! 作っている限り毎回同じ値になる（複数作るとキーが変わり、昇格は引き継がれない）。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_SAM_FLAGS, RRF_RT_REG_DWORD,
    RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::{GUID, PCWSTR, PWSTR, w};

/// 各アイコンの表示設定が並ぶ場所。
const SETTINGS_KEY: PCWSTR = w!("Control Panel\\NotifyIconSettings");
/// そのアイコンを出したプロセスの実行ファイル。
const EXECUTABLE_PATH: PCWSTR = w!("ExecutablePath");
/// 1 なら通知領域に常時表示、0 ならオーバーフロー行き。
const IS_PROMOTED: PCWSTR = w!("IsPromoted");

/// レジストリのキー名は255文字まで。
const MAX_KEY_NAME: usize = 256;

/// アイコンを出してから Windows がエントリを作るまでの待ち。
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const POLL_TIMEOUT: Duration = Duration::from_secs(10);

/// 走査の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// 値が無かったので 1 を書いた。
    Promoted,
    /// すでに設定済み。利用者の選択かもしれないので触らない。
    LeftAlone,
    /// 自分のエントリがまだ無い。Windows が作るのを待つ。
    NotRegisteredYet,
}

/// 昇格を試みる。トレイアイコンを作った直後に呼ぶ。
///
/// エントリができるのはアイコンが実際に表示されてからなので、
/// 少しのあいだ待つ必要がある。呼び出し元はメッセージループを回しており
/// 止められないため、待つのは専用スレッドに任せる。
pub fn promote_in_background() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            log::warn!("実行ファイルのパスを取得できません: {error}");
            return;
        }
    };

    let spawned = std::thread::Builder::new()
        .name("minos-tray-promotion".into())
        .spawn(move || wait_and_promote(&exe));
    if let Err(error) = spawned {
        log::warn!("トレイアイコンの昇格を開始できません: {error}");
    }
}

fn wait_and_promote(exe: &Path) {
    let deadline = Instant::now() + POLL_TIMEOUT;

    loop {
        match promote_once(exe) {
            Ok(Outcome::Promoted) => {
                // 反映は Windows が設定を読み直すまで待つ。次回の起動から見えるようになる。
                log::info!("トレイアイコンを通知領域に表示する設定にしました");
                return;
            }
            Ok(Outcome::LeftAlone) => {
                log::debug!("トレイアイコンの表示設定はすでにあります");
                return;
            }
            Ok(Outcome::NotRegisteredYet) => {
                if Instant::now() >= deadline {
                    log::debug!("トレイアイコンの表示設定がまだ作られていません");
                    return;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                log::warn!("トレイアイコンの表示設定を変更できません: {error:#}");
                return;
            }
        }
    }
}

/// `NotifyIconSettings` を1周して、自分の exe のエントリを探す。
fn promote_once(exe: &Path) -> Result<Outcome> {
    let settings = Key::open(HKEY_CURRENT_USER, SETTINGS_KEY, KEY_READ)?;

    let mut index = 0;
    loop {
        let Some(subkey) = settings.subkey_name(index)? else {
            return Ok(Outcome::NotRegisteredYet);
        };
        index += 1;

        let Some(stored) = settings.string(&subkey, EXECUTABLE_PATH) else {
            continue;
        };
        let Some(path) = resolve_executable_path(&stored) else {
            continue;
        };
        if !path.as_os_str().eq_ignore_ascii_case(exe.as_os_str()) {
            continue;
        }

        if settings.has_value(&subkey, IS_PROMOTED) {
            return Ok(Outcome::LeftAlone);
        }

        let entry = Key::open(settings.0, PCWSTR(subkey.as_ptr()), KEY_SET_VALUE)?;
        entry.set_dword(IS_PROMOTED, 1)?;
        return Ok(Outcome::Promoted);
    }
}

/// 記録されたパスを実際のパスに戻す。
///
/// 既知フォルダ（Program Files など）配下の exe は、フォルダを移しても追えるよう
/// `{KNOWNFOLDERID}\相対パス` の形で記録される。それ以外はそのままのパス。
fn resolve_executable_path(stored: &str) -> Option<PathBuf> {
    let Some(rest) = stored.strip_prefix('{') else {
        return Some(PathBuf::from(stored));
    };

    let (id, tail) = rest.split_once('}')?;
    let folder = known_folder(&GUID::try_from(id).ok()?)?;
    Some(folder.join(tail.trim_start_matches('\\')))
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let resolved = path.to_string().ok();
        // 呼び出し側が解放する契約。
        CoTaskMemFree(Some(path.as_ptr().cast()));
        resolved.map(PathBuf::from)
    }
}

/// 閉じ忘れないための `HKEY` の持ち手。
struct Key(HKEY);

impl Key {
    fn open(parent: HKEY, path: PCWSTR, access: REG_SAM_FLAGS) -> Result<Self> {
        let mut key = HKEY::default();
        let status = unsafe { RegOpenKeyExW(parent, path, None, access, &mut key) };
        if status != ERROR_SUCCESS {
            bail!("レジストリキーを開けません: {status:?}");
        }
        Ok(Self(key))
    }

    /// `index` 番目のサブキー名。NUL 終端付きで返す（そのまま `PCWSTR` にできる）。
    fn subkey_name(&self, index: u32) -> Result<Option<Vec<u16>>> {
        let mut name = [0u16; MAX_KEY_NAME];
        let mut length = name.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                self.0,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };

        match status {
            ERROR_SUCCESS => {
                let mut subkey = name[..length as usize].to_vec();
                subkey.push(0);
                Ok(Some(subkey))
            }
            ERROR_NO_MORE_ITEMS => Ok(None),
            other => bail!("サブキーを列挙できません: {other:?}"),
        }
    }

    fn string(&self, subkey: &[u16], value: PCWSTR) -> Option<String> {
        let subkey = PCWSTR(subkey.as_ptr());
        let mut bytes = 0u32;
        let status = unsafe {
            RegGetValueW(
                self.0,
                subkey,
                value,
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut bytes),
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }

        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        let mut bytes = (buffer.len() * size_of::<u16>()) as u32;
        let status = unsafe {
            RegGetValueW(
                self.0,
                subkey,
                value,
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }

        // 返る長さは NUL を含む。
        let chars = (bytes as usize / size_of::<u16>()).saturating_sub(1);
        Some(String::from_utf16_lossy(&buffer[..chars]))
    }

    fn has_value(&self, subkey: &[u16], value: PCWSTR) -> bool {
        let mut data = 0u32;
        let mut bytes = size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                self.0,
                PCWSTR(subkey.as_ptr()),
                value,
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut data).cast()),
                Some(&mut bytes),
            )
        };
        status == ERROR_SUCCESS
    }

    fn set_dword(&self, value: PCWSTR, data: u32) -> Result<()> {
        let status =
            unsafe { RegSetValueExW(self.0, value, None, REG_DWORD, Some(&data.to_ne_bytes())) };
        if status != ERROR_SUCCESS {
            bail!("レジストリ値を書けません: {status:?}");
        }
        Ok(())
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths_are_used_as_is() {
        assert_eq!(
            resolve_executable_path(r"D:\workspace\minos\target\debug\minos.exe"),
            Some(PathBuf::from(r"D:\workspace\minos\target\debug\minos.exe"))
        );
    }

    #[test]
    fn known_folder_paths_are_expanded() {
        // {6D809377-...} は FOLDERID_ProgramFilesX64。
        let resolved =
            resolve_executable_path(r"{6D809377-6AF0-444B-8957-A3773F02200E}\lineage\minos.exe")
                .expect("既知フォルダを解決できませんでした");

        assert!(resolved.ends_with(r"lineage\minos.exe"));
        assert!(resolved.is_absolute());
    }

    #[test]
    fn a_broken_known_folder_id_is_ignored() {
        assert_eq!(resolve_executable_path(r"{not-a-guid}\minos.exe"), None);
        assert_eq!(resolve_executable_path("{壊れている"), None);
    }

    /// 実レジストリを走査する。書き込みはしない。
    ///
    /// 存在しない exe が昇格されないことの確認と、列挙・値読み出しが
    /// 通ること自体の確認を兼ねる。トレイアイコンを一度も出したことがない
    /// マシンではキーごと無いので、その場合の `Err` も許す。
    #[test]
    fn an_unknown_executable_is_never_promoted() {
        let outcome = promote_once(Path::new(r"C:\nowhere\minos-does-not-live-here.exe"));
        assert!(
            matches!(outcome, Ok(Outcome::NotRegisteredYet) | Err(_)),
            "想定外の結果: {outcome:?}"
        );
    }
}
