//! 添付画像の検証・ローカルコピー。
use crate::domain::note::ImageAttachment;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn prepare_images(
    paths: &[PathBuf],
    directory: &Path,
) -> Result<(Vec<ImageAttachment>, Vec<PathBuf>)> {
    for source in paths {
        if !source.is_file() || !is_supported_image(source) {
            bail!("対応していない画像です: {}", source.display());
        }
    }

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

/// コピーの所有権は保存の成功時に確定する。失敗時はコピーだけを削除する。
pub fn with_images<T>(
    paths: &[PathBuf],
    save: impl FnOnce(Vec<ImageAttachment>) -> Result<T>,
) -> Result<T> {
    let directory = dirs::data_local_dir()
        .context("ローカルアプリケーションデータのディレクトリを特定できません")?
        .join("minos")
        .join("attachments");
    with_images_in_directory(paths, &directory, save)
}

fn with_images_in_directory<T>(
    paths: &[PathBuf],
    directory: &Path,
    save: impl FnOnce(Vec<ImageAttachment>) -> Result<T>,
) -> Result<T> {
    let (images, copied_paths) = prepare_images(paths, directory)?;
    let result = save(images);
    if result.is_err() {
        for path in copied_paths {
            _ = std::fs::remove_file(path);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_save_removes_copies_and_preserves_the_original() {
        let root = std::env::temp_dir().join(format!("minos-attachments-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("original.png");
        std::fs::write(&source, b"image fixture").unwrap();
        let destination = root.join("attachments");
        let result: Result<()> =
            with_images_in_directory(&[source.clone()], &destination, |images| {
                assert!(Path::new(&images[0].blob_uri).is_file());
                anyhow::bail!("database save failed")
            });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"image fixture");
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_save_keeps_the_copied_image() {
        let root = std::env::temp_dir().join(format!("minos-attachments-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("original.png");
        std::fs::write(&source, b"image fixture").unwrap();
        let destination = root.join("attachments");
        let copied = with_images_in_directory(&[source], &destination, |images| {
            Ok(images[0].blob_uri.clone())
        })
        .unwrap();
        assert_eq!(std::fs::read(copied).unwrap(), b"image fixture");
        std::fs::remove_dir_all(root).unwrap();
    }
}
