//! エージェント CLI（Codex / GitHub Copilot / Gemini CLI / Claude Code）へ skill を配る。
//!
//! 配る中身と配布先の一覧は webview 側の domain が持っている（core/domain/skill/）。
//! ここが持つのは「ホームの下の、この相対パスへ書く」という実行だけ。
//!
//! パスの検査はここで行う。相対パスの要素は webview から渡ってくるので、
//! `..` や区切り文字を含む要素を素通しすると、ホームの外へ書けてしまう。
//! 検査を webview 側に置くと、そこを通さない呼び出しで抜けられる。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// 版を記録するファイル名。core/domain/skill/AgentSkill.ts の SKILL_VERSION_FILE と対応する。
const VERSION_FILE: &str = "version.json";

/// 走査したい配布先1つぶん。パスはいずれもホームディレクトリからの相対。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLocation {
    id: String,
    directory: Vec<String>,
    marker: Vec<String>,
}

/// 走査の結果1件ぶん。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillScan {
    id: String,
    agent_present: bool,
    installed_version: Option<String>,
    path: String,
}

/// 書き込むファイル1つぶん。
#[derive(Deserialize)]
pub struct SkillFile {
    name: String,
    content: String,
}

/// 書き込み1件ぶん。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstall {
    id: String,
    directory: Vec<String>,
    files: Vec<SkillFile>,
}

/// 配布先を走査し、エージェントの有無と入っている skill の版を返す。
#[tauri::command]
pub async fn agent_skill_scan(
    app: AppHandle,
    locations: Vec<SkillLocation>,
) -> Result<Vec<SkillScan>, String> {
    let home = home_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        locations
            .into_iter()
            .map(|location| scan_one(&home, location))
            .collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// skill を書き込む。ディレクトリが無ければ作る。
#[tauri::command]
pub async fn agent_skill_install(
    app: AppHandle,
    installs: Vec<SkillInstall>,
) -> Result<(), String> {
    let home = home_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        for install in installs {
            install_one(&home, install)?;
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// skill 本文に埋め込む agentos の場所。
///
/// 見つからない環境（agentos をビルドしていない開発中など）でも skill 自体は配れたほうが
/// よいので、エラーにはせず実行ファイル名だけを返す。PATH 上にあれば動く。
#[tauri::command]
pub async fn agent_skill_agentos_path(app: AppHandle) -> Result<String, String> {
    Ok(crate::automation::agentos_path(&app)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "agentos.exe".to_string()
            } else {
                "agentos".to_string()
            }
        }))
}

fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|error| format!("ホームディレクトリを特定できません: {error}"))
}

fn scan_one(home: &Path, location: SkillLocation) -> Result<SkillScan, String> {
    let directory = resolve(home, &location.directory)?;
    let marker = resolve(home, &location.marker)?;
    Ok(SkillScan {
        id: location.id,
        agent_present: marker.exists(),
        installed_version: installed_version(&directory),
        path: directory.display().to_string(),
    })
}

fn install_one(home: &Path, install: SkillInstall) -> Result<(), String> {
    // 失敗はダイアログにそのまま出る。どのエージェントで起きたのか分かるよう id を添える。
    let id = &install.id;
    let directory = resolve(home, &install.directory)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("{id}: {} を作成できません: {error}", directory.display()))?;

    for file in &install.files {
        let path = resolve(&directory, std::slice::from_ref(&file.name))?;
        fs::write(&path, &file.content)
            .map_err(|error| format!("{id}: {} を書き込めません: {error}", path.display()))?;
    }
    Ok(())
}

/// 置かれている `version.json` から版を読む。
///
/// 無い・壊れている・`version` が文字列でない、のいずれも「版が分からない」＝ None にする。
/// 呼び出し側（domain の skillState）はこれを未導入と同じ扱いにし、配り直す。
fn installed_version(directory: &Path) -> Option<String> {
    let text = fs::read_to_string(directory.join(VERSION_FILE)).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("version")?.as_str().map(str::to_owned)
}

/// 相対パスの要素を基点に結合する。
///
/// 要素は「1つのディレクトリ名／ファイル名」でなければならない。空・`.`・`..`・
/// 区切り文字を含むものを弾くことで、結合の結果が必ず基点の下に収まるようにする。
fn resolve(base: &Path, segments: &[String]) -> Result<PathBuf, String> {
    if segments.is_empty() {
        return Err("配置先が指定されていません".to_string());
    }

    let mut path = base.to_path_buf();
    for segment in segments {
        let invalid = segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('/')
            || segment.contains('\\')
            // Windows のドライブ指定（`C:`）とデータストリーム（`a:b`）を弾く。
            || segment.contains(':');
        if invalid {
            return Err(format!("配置先として扱えない名前です: {segment}"));
        }
        path.push(segment);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_joins_segments_under_the_base() {
        let path = resolve(Path::new("/home/u"), &[".codex".into(), "skills".into()]).unwrap();
        assert_eq!(path, Path::new("/home/u/.codex/skills"));
    }

    #[test]
    fn resolve_rejects_escaping_segments() {
        for segment in ["..", ".", "", "a/b", "a\\b", "C:"] {
            let result = resolve(Path::new("/home/u"), &[segment.to_string()]);
            assert!(result.is_err(), "{segment} should be rejected");
        }
    }

    #[test]
    fn resolve_rejects_an_empty_path() {
        assert!(resolve(Path::new("/home/u"), &[]).is_err());
    }

    #[test]
    fn installed_version_reads_the_version_field() {
        let dir = std::env::temp_dir().join("lineage-skill-test-version");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(VERSION_FILE), r#"{"name":"lineage","version":"1.2.3"}"#).unwrap();
        assert_eq!(installed_version(&dir), Some("1.2.3".to_string()));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn installed_version_is_none_when_missing_or_broken() {
        let dir = std::env::temp_dir().join("lineage-skill-test-broken");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(installed_version(&dir), None);

        fs::write(dir.join(VERSION_FILE), "{ not json").unwrap();
        assert_eq!(installed_version(&dir), None);

        fs::write(dir.join(VERSION_FILE), r#"{"version":3}"#).unwrap();
        assert_eq!(installed_version(&dir), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_one_writes_files_under_the_home() {
        let home = std::env::temp_dir().join("lineage-skill-test-install");
        let _ = fs::remove_dir_all(&home);
        install_one(
            &home,
            SkillInstall {
                id: "codex".into(),
                directory: vec![".codex".into(), "skills".into(), "lineage".into()],
                files: vec![SkillFile {
                    name: "SKILL.md".into(),
                    content: "body".into(),
                }],
            },
        )
        .unwrap();

        let written = home.join(".codex/skills/lineage/SKILL.md");
        assert_eq!(fs::read_to_string(&written).unwrap(), "body");
        fs::remove_dir_all(&home).unwrap();
    }
}
