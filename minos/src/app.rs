//! composition root。
//!
//! 具体的な実装（SQLite / SHA-256 / システム時計）をユースケースに注入し、
//! features 層（画面）には「何ができるか」だけを見せる。
//! features はここより内側の実装を直接知らない。

use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result, bail};

use lineage_core::app::capture::{CaptureMemo, CaptureMemoInput, CaptureMemoOutput};
use lineage_core::app::lineage::VerifyLineage;
use lineage_core::app::meta::CompleteMetaTag;
use lineage_core::app::settings::{LoadSettings, SaveSettings};
use lineage_core::domain::automation::MemoSnapshot;
use lineage_core::domain::capture::{CaptureContext, ImageAttachment};
use lineage_core::domain::lineage::VerifyResult;
use lineage_core::domain::meta::{MetaAssignment, MetaSuggestion};
use lineage_core::domain::ports::MemoQuery;
use lineage_core::domain::settings::Settings;
use lineage_core::infra::clock::{SystemClock, UuidGenerator};
use lineage_core::infra::crypto::Sha256Hasher;
use lineage_core::infra::sqlite::Database;

/// minos は単一利用者なので、既定のワークスペースは1つ固定。
/// クラウド接続に切り替えるときは、ここが利用者ごとの workspace になる。
const DEFAULT_WORKSPACE_ID: &str = "local";
const DEFAULT_WORKSPACE_NAME: &str = "minos";

pub struct Services {
    database: Database,
    clock: SystemClock,
    ids: UuidGenerator,
    hasher: Sha256Hasher,
    workspace_id: String,
}

impl Services {
    pub fn new(database: Database) -> Rc<Self> {
        Rc::new(Self {
            database,
            clock: SystemClock,
            ids: UuidGenerator,
            hasher: Sha256Hasher,
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
        })
    }

    /// 入力を1件確定する（document + lineage を同一トランザクションで保存）。
    pub fn capture(
        &self,
        body: String,
        metas: Vec<MetaAssignment>,
        context: Option<CaptureContext>,
        document_id: Option<String>,
        image_paths: Vec<PathBuf>,
    ) -> Result<CaptureMemoOutput> {
        let (images, copied_paths) = prepare_images(&image_paths)?;
        let result = CaptureMemo::new(&self.database, &self.clock, &self.ids, &self.hasher)
            .execute(CaptureMemoInput {
                workspace_id: self.workspace_id.clone(),
                workspace_name: DEFAULT_WORKSPACE_NAME.to_string(),
                body,
                document_id,
                metas,
                context,
                images,
            });
        if result.is_err() {
            for path in copied_paths {
                _ = std::fs::remove_file(path);
            }
        }
        result
    }

    pub fn recent_memos(&self, limit: usize) -> Result<Vec<MemoSnapshot>> {
        self.database.recent(&self.workspace_id, limit)
    }

    pub fn memo(&self, document_id: &str) -> Result<Option<MemoSnapshot>> {
        self.database.get(&self.workspace_id, document_id)
    }

    /// `#` の入力補完候補。
    pub fn suggest_meta_tags(&self, query: &str, limit: usize) -> Result<Vec<MetaSuggestion>> {
        CompleteMetaTag::new(&self.database).execute(&self.workspace_id, query, limit)
    }

    /// 保存されている設定（未保存なら既定値）。
    pub fn load_settings(&self) -> Result<Settings> {
        LoadSettings::new(&self.database).execute(&self.workspace_id)
    }

    /// 設定を保存する。fullos の設定画面からも同じ行を編集する。
    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        SaveSettings::new(&self.database, &self.clock).execute(&self.workspace_id, settings)
    }

    /// hash-chain の検証。
    pub fn verify_lineage(&self) -> Result<VerifyResult> {
        VerifyLineage::new(&self.database, &self.hasher).execute(&self.workspace_id)
    }
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn prepare_images(paths: &[PathBuf]) -> Result<(Vec<ImageAttachment>, Vec<PathBuf>)> {
    for source in paths {
        if !source.is_file() || !is_supported_image(source) {
            bail!("対応していない画像です: {}", source.display());
        }
    }

    let directory = dirs::data_local_dir()
        .context("ローカルアプリケーションデータのディレクトリを特定できません")?
        .join("minos")
        .join("attachments");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("画像の保存先を作成できません: {}", directory.display()))?;

    let mut images = Vec::with_capacity(paths.len());
    let mut copied = Vec::with_capacity(paths.len());
    for source in paths {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image");
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("png");
        let destination = directory.join(format!("{}.{}", uuid::Uuid::new_v4(), extension));
        if let Err(error) = std::fs::copy(source, &destination) {
            for path in copied {
                _ = std::fs::remove_file(path);
            }
            return Err(error)
                .with_context(|| format!("画像を添付できません: {}", source.display()));
        }
        images.push(ImageAttachment {
            name: name.to_string(),
            blob_uri: destination.to_string_lossy().into_owned(),
        });
        copied.push(destination);
    }
    Ok((images, copied))
}
