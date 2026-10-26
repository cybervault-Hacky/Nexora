use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::inference::hardware;

use super::error::{ImageEditingError as EditError, Result};

const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 2_048;
const MAX_PIXELS: u64 = 4 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHUNKS: usize = 4_096;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportImageEditSourceRequest {
    /// Raw bytes from a user-selected file. No renderer-supplied path is used.
    pub source_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEditSourceArtifact {
    pub schema_version: u16,
    pub source_id: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub sha256: String,
    pub created_at_unix_ms: u64,
}

pub(crate) struct SourceImageStore {
    root: PathBuf,
}

struct StagingDirectory(PathBuf);

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::symlink_metadata(&self.0) {
            if !metadata.file_type().is_symlink() && metadata.is_dir() {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
}

impl SourceImageStore {
    pub fn new(root: PathBuf) -> Self { Self { root } }

    pub fn root(&self) -> &Path { &self.root }

    pub fn import(&self, request: ImportImageEditSourceRequest) -> Result<ImageEditSourceArtifact> {
        let (width, height) = validate_png(&request.source_bytes)?;
        let needed = (request.source_bytes.len() as u64).saturating_add(64 * 1024);
        if hardware::disk_space(&self.root).free_bytes.map_or(true, |free| free < needed) {
            return Err(EditError::insufficient_storage());
        }
        self.ensure_layout()?;
        let source_id = random_id()?;
        let staging_dir = self.root.join(".staging").join(&source_id);
        fs::create_dir(&staging_dir).map_err(|_| EditError::storage("A private source-image staging folder could not be created."))?;
        let _staging_guard = StagingDirectory(staging_dir.clone());

        let staged_image = staging_dir.join("source.png");
        let staged_record = staging_dir.join("source.json");
        write_new_synced(&staged_image, &request.source_bytes)?;
        let artifact = ImageEditSourceArtifact {
            schema_version: 1,
            source_id: source_id.clone(),
            mime_type: "image/png".to_string(),
            width,
            height,
            bytes: request.source_bytes.len() as u64,
            sha256: sha256_hex(&request.source_bytes),
            created_at_unix_ms: now_ms(),
        };
        let encoded = serde_json::to_vec(&artifact)
            .map_err(|_| EditError::storage("Source-image metadata could not be encoded."))?;
        write_new_synced(&staged_record, &encoded)?;

        let sources = self.root.join("sources");
        let final_image = sources.join(format!("{source_id}.png"));
        let final_record = sources.join(format!("{source_id}.json"));
        fs::hard_link(&staged_image, &final_image)
            .map_err(|_| EditError::storage("The source image could not be published without replacing an existing source."))?;
        if fs::hard_link(&staged_record, &final_record).is_err() {
            let _ = fs::remove_file(&final_image);
            return Err(EditError::storage("Source-image metadata could not be published without replacing an existing source."));
        }
        Ok(artifact)
    }

    /// Deletes one user-selected source by opaque ID only. No path is accepted.
    pub fn remove(&self, source_id: &str) -> Result<()> {
        if !valid_id(source_id) { return Err(EditError::unknown_source()); }
        let root_meta = fs::symlink_metadata(&self.root).map_err(|_| EditError::unknown_source())?;
        if root_meta.file_type().is_symlink() || !root_meta.is_dir() { return Err(EditError::unknown_source()); }
        let sources = self.root.join("sources");
        let sources_meta = fs::symlink_metadata(&sources).map_err(|_| EditError::unknown_source())?;
        if sources_meta.file_type().is_symlink() || !sources_meta.is_dir() { return Err(EditError::unknown_source()); }
        let image_path = sources.join(format!("{source_id}.png"));
        let record_path = sources.join(format!("{source_id}.json"));
        for path in [&image_path, &record_path] {
            let metadata = fs::symlink_metadata(path).map_err(|_| EditError::unknown_source())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(EditError::unknown_source());
            }
        }
        fs::remove_file(&image_path)
            .map_err(|_| EditError::storage("The managed source image could not be removed."))?;
        fs::remove_file(&record_path)
            .map_err(|_| EditError::storage("The managed source metadata could not be removed."))?;
        Ok(())
    }

    pub fn get(&self, source_id: &str) -> Result<ImageEditSourceArtifact> {
        if !valid_id(source_id) { return Err(EditError::unknown_source()); }
        let root_meta = fs::symlink_metadata(&self.root).map_err(|_| EditError::unknown_source())?;
        if root_meta.file_type().is_symlink() || !root_meta.is_dir() { return Err(EditError::unknown_source()); }
        let sources = self.root.join("sources");
        let sources_meta = fs::symlink_metadata(&sources).map_err(|_| EditError::unknown_source())?;
        if sources_meta.file_type().is_symlink() || !sources_meta.is_dir() { return Err(EditError::unknown_source()); }

        let image_path = sources.join(format!("{source_id}.png"));
        let record_path = sources.join(format!("{source_id}.json"));
        let image_meta = fs::symlink_metadata(&image_path).map_err(|_| EditError::unknown_source())?;
        let record_meta = fs::symlink_metadata(&record_path).map_err(|_| EditError::unknown_source())?;
        if image_meta.file_type().is_symlink() || !image_meta.is_file()
            || image_meta.len() == 0 || image_meta.len() > MAX_SOURCE_BYTES as u64
            || record_meta.file_type().is_symlink() || !record_meta.is_file() || record_meta.len() > 16 * 1024
        {
            return Err(EditError::unknown_source());
        }
        let mut encoded = Vec::with_capacity(record_meta.len() as usize);
        let mut locked_record = crate::inference::models::LockedFile::open(&record_path)
            .map_err(|_| EditError::unknown_source())?;
        let locked_record_len = locked_record.file().metadata()
            .map_err(|_| EditError::unknown_source())?.len();
        if locked_record_len > 16 * 1024 || locked_record_len != record_meta.len() {
            return Err(EditError::unknown_source());
        }
        locked_record.file_mut().read_to_end(&mut encoded)
            .map_err(|_| EditError::unknown_source())?;
        if encoded.len() as u64 != locked_record_len { return Err(EditError::unknown_source()); }
        let artifact: ImageEditSourceArtifact = serde_json::from_slice(&encoded)
            .map_err(|_| EditError::unknown_source())?;
        if artifact.schema_version != 1 || artifact.source_id != source_id
            || artifact.mime_type != "image/png" || artifact.bytes != image_meta.len()
        {
            return Err(EditError::unknown_source());
        }
        let mut bytes = Vec::with_capacity(image_meta.len() as usize);
        let mut locked_image = crate::inference::models::LockedFile::open(&image_path)
            .map_err(|_| EditError::unknown_source())?;
        let locked_image_len = locked_image.file().metadata()
            .map_err(|_| EditError::unknown_source())?.len();
        if locked_image_len == 0 || locked_image_len > MAX_SOURCE_BYTES as u64
            || locked_image_len != image_meta.len()
        {
            return Err(EditError::unknown_source());
        }
        locked_image.file_mut().read_to_end(&mut bytes)
            .map_err(|_| EditError::unknown_source())?;
        if bytes.len() as u64 != locked_image_len { return Err(EditError::integrity()); }
        let (width, height) = validate_png(&bytes).map_err(|_| EditError::integrity())?;
        if width != artifact.width || height != artifact.height
            || !sha256_hex(&bytes).eq_ignore_ascii_case(&artifact.sha256)
        {
            return Err(EditError::integrity());
        }
        Ok(artifact)
    }

    fn ensure_layout(&self) -> Result<()> {
        match fs::symlink_metadata(&self.root) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(EditError::storage("The application-managed image-editing root is not a normal directory."));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&self.root).map_err(|_| EditError::storage("The image-editing storage folder could not be created."))?;
            }
            Err(_) => return Err(EditError::storage("The image-editing storage folder could not be inspected.")),
        }
        for child in [self.root.join("sources"), self.root.join(".staging")] {
            match fs::symlink_metadata(&child) {
                Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                    return Err(EditError::storage("An image-editing subfolder is not a normal directory."));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&child).map_err(|_| EditError::storage("An image-editing subfolder could not be created."))?;
                }
                Err(_) => return Err(EditError::storage("An image-editing subfolder could not be inspected.")),
            }
        }
        Ok(())
    }
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_| EditError::storage("An application-managed source file could not be created exclusively."))?;
    file.write_all(bytes).and_then(|_| file.sync_all())
        .map_err(|_| EditError::storage("An application-managed source file could not be written safely."))
}

fn validate_png(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() > MAX_SOURCE_BYTES { return Err(EditError::image_too_large()); }
    if bytes.len() < 45 || &bytes[..8] != PNG_SIGNATURE { return Err(EditError::unsupported_format()); }

    let mut offset = 8usize;
    let mut saw_ihdr = false;
    let mut saw_idat = false;
    let mut saw_iend = false;
    let mut width = 0;
    let mut height = 0;
    let mut chunks = 0usize;
    while offset.checked_add(12).map_or(false, |end| end <= bytes.len()) {
        chunks += 1;
        if chunks > MAX_CHUNKS { return Err(EditError::invalid_image()); }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| EditError::invalid_image())?) as usize;
        let data_start = offset.checked_add(8).ok_or_else(EditError::invalid_image)?;
        let data_end = data_start.checked_add(length).ok_or_else(EditError::invalid_image)?;
        let end = data_end.checked_add(4).ok_or_else(EditError::invalid_image)?;
        if end > bytes.len() { return Err(EditError::invalid_image()); }
        let kind = &bytes[offset + 4..offset + 8];
        let expected_crc = u32::from_be_bytes(bytes[data_end..end].try_into().map_err(|_| EditError::invalid_image())?);
        if crc32fast::hash(&bytes[offset + 4..data_end]) != expected_crc { return Err(EditError::invalid_image()); }
        if !saw_ihdr {
            if kind != b"IHDR" || length != 13 { return Err(EditError::invalid_image()); }
            width = u32::from_be_bytes(bytes[data_start..data_start + 4].try_into().map_err(|_| EditError::invalid_image())?);
            height = u32::from_be_bytes(bytes[data_start + 4..data_start + 8].try_into().map_err(|_| EditError::invalid_image())?);
            let bit_depth = bytes[data_start + 8];
            let color_type = bytes[data_start + 9];
            if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION
                || u64::from(width).checked_mul(u64::from(height)).map_or(true, |pixels| pixels > MAX_PIXELS)
            {
                return Err(EditError::dimensions());
            }
            if bit_depth != 8 || !matches!(color_type, 2 | 6) { return Err(EditError::unsupported_format()); }
            if bytes[data_start + 10] != 0 || bytes[data_start + 11] != 0 || bytes[data_start + 12] > 1 {
                return Err(EditError::invalid_image());
            }
            saw_ihdr = true;
        } else if kind == b"IHDR" {
            return Err(EditError::invalid_image());
        }
        if kind == b"acTL" || kind == b"fcTL" || kind == b"fdAT" {
            return Err(EditError::unsupported_format());
        }
        if kind == b"IDAT" { saw_idat = true; }
        if kind == b"IEND" {
            if length != 0 || !saw_idat || end != bytes.len() { return Err(EditError::invalid_image()); }
            saw_iend = true;
            break;
        }
        offset = end;
    }
    if !saw_ihdr || !saw_iend { return Err(EditError::invalid_image()); }

    // Decode, not just inspect, the source pixels. The output allocation is
    // bounded before next_frame to prevent malformed headers expanding memory.
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|_| EditError::invalid_image())?;
    if reader.info().width != width || reader.info().height != height {
        return Err(EditError::invalid_image());
    }
    let output_size = reader.output_buffer_size().ok_or_else(EditError::invalid_image)?;
    if output_size > MAX_DECODED_BYTES { return Err(EditError::dimensions()); }
    let mut decoded = vec![0u8; output_size];
    let frame = reader.next_frame(&mut decoded).map_err(|_| EditError::invalid_image())?;
    if frame.width != width || frame.height != height
        || !matches!(frame.color_type, png::ColorType::Rgb | png::ColorType::Rgba)
        || frame.bit_depth != png::BitDepth::Eight
    {
        return Err(EditError::unsupported_format());
    }
    Ok((width, height))
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| EditError::storage("A secure image-source identifier could not be created."))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn valid_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).ok()
        .and_then(|time| u64::try_from(time.as_millis()).ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_the_named_managed_source_pair() {
        let temp = tempfile::tempdir().expect("temp directory");
        let root = temp.path().join("image-editing");
        let sources = root.join("sources");
        fs::create_dir_all(&sources).expect("sources directory");
        let id = "0123456789abcdef0123456789abcdef";
        let image = sources.join(format!("{id}.png"));
        let record = sources.join(format!("{id}.json"));
        fs::write(&image, b"managed image bytes").expect("image file");
        fs::write(&record, b"managed metadata").expect("metadata file");

        SourceImageStore::new(root).remove(id).expect("remove source");
        assert!(!image.exists());
        assert!(!record.exists());
    }

    #[test]
    fn rejects_empty_corrupt_and_oversized_png_payloads() {
        assert!(validate_png(&[]).is_err());
        assert!(validate_png(b"not a png").is_err());
        assert!(matches!(validate_png(&vec![0u8; MAX_SOURCE_BYTES + 1]), Err(_)));
    }

    #[test]
    fn rejects_path_shaped_or_unknown_source_ids_for_removal() {
        let temp = tempfile::tempdir().expect("temp directory");
        let store = SourceImageStore::new(temp.path().join("image-editing"));
        assert!(store.remove("../outside").is_err());
        assert!(store.remove("0123456789abcdef0123456789abcdef").is_err());
    }
}
