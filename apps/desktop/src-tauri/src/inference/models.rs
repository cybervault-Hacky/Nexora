use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::error::{InferenceError, InferenceResult};

/// A read-only handle to the exact file that was opened for validation.
///
/// Windows opens use a read-only share mode: while this handle is alive, no
/// other process can write, replace, or delete the file. The runtime keeps this
/// guard for the entire child-process lifetime, closing the verify-to-open race
/// on the supported Windows desktop path. Other platforms retain the open file
/// handle but do not provide the same mandatory share-mode guarantee.
pub struct LockedFile {
    file: File,
    path: PathBuf,
}

impl LockedFile {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the model path is not a regular non-symlink file",
            ));
        }

        // Resolve relative paths and parent-directory links once, then open the
        // resolved regular file. The runtime receives this same canonical path.
        let resolved = std::fs::canonicalize(path)?;
        let resolved_metadata = std::fs::symlink_metadata(&resolved)?;
        if resolved_metadata.file_type().is_symlink() || !resolved_metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the resolved model path is not a regular non-symlink file",
            ));
        }

        let mut options = OpenOptions::new();
        options.read(true);

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

            // Do not allow write or delete sharing while the file is verified
            // and passed to llama-server. A compatible read handle remains
            // available to the runtime itself.
            options.share_mode(FILE_SHARE_READ);
        }

        let file = options.open(&resolved)?;
        let opened_metadata = file.metadata()?;
        if !opened_metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the opened model path is not a regular file",
            ));
        }

        Ok(Self {
            file,
            path: resolved,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub fn file(&self) -> &File {
        &self.file
    }
}

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const MIN_PLAUSIBLE_BYTES: u64 = 16 * 1024 * 1024;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// What the header of a GGUF file can tell us without loading the tensors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub size_gib: f64,
    pub gguf_version: u32,
    pub tensor_count: u64,
    /// `general.architecture` from the metadata table, when it could be read.
    pub architecture: Option<String>,
    /// `general.name`, the publisher's own label for the file.
    pub name: Option<String>,
    /// The context window the file itself was built for, when it declares one.
    pub context_length: Option<u32>,
    pub estimated_min_ram_gib: f64,
    pub fits_available_ram: Option<bool>,
    pub warnings: Vec<String>,
}

/// Reads the GGUF header and returns everything that can be checked cheaply.
///
/// This is an integrity and compatibility pre-check, not a full validation:
/// tensor data, tokenizer metadata, and architecture support are proven only
/// when `llama-server` actually loads the file.
pub fn inspect(
    path: &str,
    context_size: u32,
    available_ram_gib: Option<f64>,
) -> InferenceResult<ModelInfo> {
    let trimmed = path.trim();

    if trimmed.is_empty() {
        return Err(InferenceError::not_configured("Choose a model file first."));
    }

    let file_path = Path::new(trimmed);
    let extension_is_gguf = file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);

    if !extension_is_gguf {
        return Err(InferenceError::invalid_model(
            "Nexora loads GGUF model files only. Choose a file ending in .gguf.",
        ));
    }

    let mut locked = LockedFile::open(file_path).map_err(|error| {
        InferenceError::invalid_model(format!("The model file could not be opened: {error}"))
    })?;

    inspect_open_file(trimmed, locked.file_mut(), context_size, available_ram_gib)
}

/// Inspects an already-open file handle. Load-time verification and header
/// inspection use the same handle that is retained for the runtime process.
pub fn inspect_open_file(
    path: &str,
    file: &mut File,
    context_size: u32,
    available_ram_gib: Option<f64>,
) -> InferenceResult<ModelInfo> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(InferenceError::not_configured("Choose a model file first."));
    }

    let file_path = Path::new(trimmed);
    let extension_is_gguf = file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);

    if !extension_is_gguf {
        return Err(InferenceError::invalid_model(
            "Nexora loads GGUF model files only. Choose a file ending in .gguf.",
        ));
    }

    file.seek(SeekFrom::Start(0))
        .map_err(|error| InferenceError::invalid_model(format!("The model file could not be read: {error}")))?;

    let size_bytes = file
        .metadata()
        .map_err(|error| InferenceError::invalid_model(error.to_string()))?
        .len();

    if size_bytes < MIN_PLAUSIBLE_BYTES {
        return Err(InferenceError::invalid_model(
            "That file is far too small to be a language model.",
        ));
    }

    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|_| InferenceError::invalid_model("The file ended before its GGUF header did."))?;

    if &header[0..4] != GGUF_MAGIC {
        return Err(InferenceError::invalid_model(
            "The file does not start with a GGUF header, so the engine cannot load it.",
        ));
    }

    let gguf_version = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);

    if !(2..=3).contains(&gguf_version) {
        return Err(InferenceError::invalid_model(format!(
            "GGUF version {gguf_version} is not supported. Use a version 2 or 3 file."
        )));
    }

    let tensor_count = u64::from_le_bytes(header[8..16].try_into().unwrap_or([0; 8]));
    let metadata_entries = u64::from_le_bytes(header[16..24].try_into().unwrap_or([0; 8]));

    let size_gib = size_bytes as f64 / GIB;
    let mut warnings = Vec::new();

    if tensor_count == 0 {
        warnings.push("The header lists no tensors; the file may be truncated.".to_string());
    }

    // The metadata table is the only place a GGUF file declares what it is.
    // Reading it is best-effort: a file that cannot be parsed is still a file
    // the engine may load, so this warns instead of rejecting. The context
    // check below only fires when the value is actually known.
    let metadata = match read_metadata(file, metadata_entries) {
        Ok(metadata) => metadata,
        Err(reason) => {
            warnings.push(format!(
                "The model metadata could not be read ({reason}). The file may be truncated, and its compatibility cannot be checked."
            ));
            GgufMetadata::default()
        }
    };

    if let Some(maximum) = metadata.context_length {
        if context_size > maximum {
            warnings.push(format!(
                "This file reports a maximum context of {maximum} tokens, but Nexora is set to {context_size}. Lower the context window before loading."
            ));
        }
    }

    // A deliberately blunt planning figure: weights, a fixed runtime reserve,
    // and a context allowance. It is an estimate, not a measurement, and it
    // excludes architecture-specific KV growth beyond the allowance.
    let context_allowance = 0.5 + (context_size as f64 / 32_768.0) * 1.5;
    let estimated_min_ram_gib = size_gib + context_allowance + 1.0;

    let fits_available_ram = available_ram_gib.map(|available| {
        // Leave the desktop at least 2 GiB of headroom.
        estimated_min_ram_gib <= (available - 2.0).max(0.0)
    });

    if fits_available_ram == Some(false) {
        warnings.push(
            "This model looks larger than the memory Nexora can see. Loading it may spill to disk and become very slow."
                .to_string(),
        );
    }

    Ok(ModelInfo {
        path: trimmed.to_string(),
        file_name: file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("model.gguf")
            .to_string(),
        size_bytes,
        size_gib: round2(size_gib),
        gguf_version,
        tensor_count,
        architecture: metadata.architecture,
        name: metadata.name,
        context_length: metadata.context_length,
        estimated_min_ram_gib: round2(estimated_min_ram_gib),
        fits_available_ram,
        warnings,
    })
}

/// The subset of GGUF metadata Nexora reads before the engine loads a file.
///
/// GGUF stores a flat key/value table behind the 24-byte header. Only three
/// values matter to the product: the architecture, a display name, and the
/// context window the file declares. Everything else is skipped by length, so
/// a multi-megabyte tokenizer table costs a few reads rather than an allocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GgufMetadata {
    pub architecture: Option<String>,
    pub name: Option<String>,
    pub context_length: Option<u32>,
}

/// Ceiling on the metadata table. A corrupt entry count must not make Nexora
/// read the whole 4.7 GiB file looking for a key that is not there.
const MAX_METADATA_BYTES: u64 = 32 * 1024 * 1024;
const MAX_METADATA_ENTRIES: u64 = 200_000;
const MAX_KEY_BYTES: u64 = 4_096;
const MAX_LABEL_BYTES: u64 = 4_096;

/// GGUF metadata value types, from the format specification.
mod gguf_type {
    pub const UINT8: u32 = 0;
    pub const INT8: u32 = 1;
    pub const UINT16: u32 = 2;
    pub const INT16: u32 = 3;
    pub const UINT32: u32 = 4;
    pub const INT32: u32 = 5;
    pub const FLOAT32: u32 = 6;
    pub const BOOL: u32 = 7;
    pub const STRING: u32 = 8;
    pub const ARRAY: u32 = 9;
    pub const UINT64: u32 = 10;
    pub const INT64: u32 = 11;
    pub const FLOAT64: u32 = 12;
}

/// A reader that refuses to consume more than the metadata budget.
struct BoundedReader<'a, R: Read> {
    inner: &'a mut R,
    remaining: u64,
}

impl<'a, R: Read> BoundedReader<'a, R> {
    fn new(inner: &'a mut R) -> Self {
        Self {
            inner,
            remaining: MAX_METADATA_BYTES,
        }
    }

    fn take(&mut self, count: u64) -> Result<Vec<u8>, String> {
        if count > self.remaining {
            return Err("the metadata table is larger than Nexora reads".to_string());
        }

        // `count` is bounded by the 32 MiB budget above, so this cannot
        // allocate more than that.
        let mut buffer = vec![0u8; count as usize];

        self.inner
            .read_exact(&mut buffer)
            .map_err(|_| "the file ended inside its metadata table".to_string())?;

        self.remaining -= count;

        Ok(buffer)
    }

    fn skip(&mut self, count: u64) -> Result<(), String> {
        if count > self.remaining {
            return Err("the metadata table is larger than Nexora reads".to_string());
        }

        let mut left = count;
        let mut scratch = [0u8; 8192];

        while left > 0 {
            let step = left.min(scratch.len() as u64) as usize;

            self.inner
                .read_exact(&mut scratch[..step])
                .map_err(|_| "the file ended inside its metadata table".to_string())?;

            left -= step as u64;
            self.remaining -= step as u64;
        }

        Ok(())
    }

    fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        let mut value = [0u8; 4];
        value.copy_from_slice(&bytes);

        Ok(u32::from_le_bytes(value))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let bytes = self.take(8)?;
        let mut value = [0u8; 8];
        value.copy_from_slice(&bytes);

        Ok(u64::from_le_bytes(value))
    }

    fn text(&mut self, count: u64, maximum: u64) -> Result<String, String> {
        if count > maximum {
            return Err(format!("a metadata string claims {count} bytes"));
        }

        let bytes = self.take(count)?;

        String::from_utf8(bytes).map_err(|_| "a metadata string is not valid UTF-8".to_string())
    }

    fn skip_many(&mut self, count: u64, size: u64) -> Result<(), String> {
        let total = count
            .checked_mul(size)
            .ok_or_else(|| "a metadata array is too large".to_string())?;

        self.skip(total)
    }

    /// Skips a value whose type is known but whose contents are not used.
    fn skip_value(&mut self, kind: u32) -> Result<(), String> {
        match kind {
            gguf_type::UINT8 | gguf_type::INT8 | gguf_type::BOOL => self.skip(1),
            gguf_type::UINT16 | gguf_type::INT16 => self.skip(2),
            gguf_type::UINT32 | gguf_type::INT32 | gguf_type::FLOAT32 => self.skip(4),
            gguf_type::UINT64 | gguf_type::INT64 | gguf_type::FLOAT64 => self.skip(8),
            gguf_type::STRING => {
                let length = self.u64()?;
                self.skip(length)
            }
            gguf_type::ARRAY => {
                let element = self.u32()?;
                let count = self.u64()?;

                match element {
                    gguf_type::STRING => {
                        // Every element carries at least an 8-byte length.
                        if count > self.remaining / 8 {
                            return Err(
                                "a metadata array claims more elements than the file can hold"
                                    .to_string(),
                            );
                        }

                        for _ in 0..count {
                            let length = self.u64()?;
                            self.skip(length)?;
                        }

                        Ok(())
                    }
                    gguf_type::UINT8 | gguf_type::INT8 | gguf_type::BOOL => {
                        self.skip_many(count, 1)
                    }
                    gguf_type::UINT16 | gguf_type::INT16 => self.skip_many(count, 2),
                    gguf_type::UINT32 | gguf_type::INT32 | gguf_type::FLOAT32 => {
                        self.skip_many(count, 4)
                    }
                    gguf_type::UINT64 | gguf_type::INT64 | gguf_type::FLOAT64 => {
                        self.skip_many(count, 8)
                    }
                    other => Err(format!(
                        "a metadata array has an unsupported element type {other}"
                    )),
                }
            }
            other => Err(format!("metadata value type {other} is not supported")),
        }
    }
}

/// Reads the metadata table that follows the GGUF header.
///
/// `entries` is the count from the header. The parser is deliberately strict
/// about lengths and types: a truncated or malformed table returns a reason
/// instead of silently reporting partial facts.
pub fn read_metadata<R: Read>(input: &mut R, entries: u64) -> Result<GgufMetadata, String> {
    if entries > MAX_METADATA_ENTRIES {
        return Err(format!("the header claims {entries} metadata entries"));
    }

    let mut reader = BoundedReader::new(input);
    let mut metadata = GgufMetadata::default();

    for _ in 0..entries {
        let key_length = reader.u64()?;
        let key = reader.text(key_length, MAX_KEY_BYTES)?;
        let kind = reader.u32()?;

        match kind {
            gguf_type::STRING => {
                let length = reader.u64()?;

                match key.as_str() {
                    "general.architecture" => {
                        metadata.architecture = Some(reader.text(length, MAX_LABEL_BYTES)?)
                    }
                    "general.name" => metadata.name = Some(reader.text(length, MAX_LABEL_BYTES)?),
                    _ => reader.skip(length)?,
                }
            }
            gguf_type::UINT32 => {
                let value = reader.u32()?;

                if is_context_key(&key) {
                    metadata.context_length = Some(value);
                }
            }
            gguf_type::UINT64 => {
                let value = reader.u64()?;

                if is_context_key(&key) {
                    metadata.context_length = Some(value.min(u32::MAX as u64) as u32);
                }
            }
            other => reader.skip_value(other)?,
        }
    }

    Ok(metadata)
}

/// `<architecture>.context_length` is the per-model key GGUF uses.
fn is_context_key(key: &str) -> bool {
    key.len() > ".context_length".len() && key.ends_with(".context_length")
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Reads a bounded prefix of the runtime executable so a wrong file is caught
/// before it is ever launched.
pub fn looks_like_executable(path: &str) -> InferenceResult<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(InferenceError::not_configured(
            "Choose the llama-server executable before loading.",
        ));
    }

    let file_path = Path::new(trimmed);

    if !file_path.exists() {
        return Err(InferenceError::runtime_missing(format!(
            "No runtime executable exists at {trimmed}."
        )));
    }

    if !file_path.is_file() {
        return Err(InferenceError::incompatible_runtime(format!(
            "The runtime path {trimmed} is not a regular file."
        )));
    }

    let mut file = File::open(file_path)?;
    let mut magic = [0u8; 4];
    let read = read_magic(&mut file, &mut magic)?;

    if read < 2 {
        return Err(InferenceError::incompatible_runtime(
            "That runtime file is empty.",
        ));
    }

    let is_windows_binary = &magic[..2] == b"MZ";
    let is_script = &magic[..2] == b"#!";
    // Nexora ships for Windows, but the same code runs on a Linux development
    // machine, where `llama-server` is an ELF binary. Accepting both keeps the
    // check honest on each platform instead of calling an ELF file "not a
    // Windows executable" on Linux.
    let is_elf = read >= 4 && &magic[..4] == b"\x7fELF" && cfg!(unix);
    let runs_here = (is_windows_binary && cfg!(windows)) || (is_script && cfg!(unix)) || is_elf;

    if !runs_here {
        return Err(InferenceError::incompatible_runtime(if cfg!(windows) {
            "That file is not a Windows executable. Choose llama-server.exe."
        } else {
            "That file is not an executable for this platform. Choose the llama-server binary."
        }));
    }

    // Reject a directory masquerading as a file and confirm we can seek.
    file.seek(SeekFrom::Start(0))
        .map_err(|error| InferenceError::incompatible_runtime(error.to_string()))?;

    Ok(())
}

/// Reads up to `buffer.len()` leading bytes, tolerating short reads.
fn read_magic(file: &mut File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;

    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) => return Err(error),
        }
    }

    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("nexora-model-tests");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).expect("create");
        file.write_all(bytes).expect("write");
        path
    }

    fn gguf_header(version: u32, tensors: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&tensors.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.resize((MIN_PLAUSIBLE_BYTES + 1) as usize, 0);
        bytes
    }

    /// One metadata value, written exactly as the GGUF specification lays it out.
    #[derive(Clone)]
    enum Value<'a> {
        Text(&'a str),
        U32(u32),
        U64(u64),
        Bool(bool),
        F32(f32),
        TextArray(&'a [&'a str]),
        U32Array(&'a [u32]),
    }

    fn push_kv(bytes: &mut Vec<u8>, key: &str, value: Value<'_>) {
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key.as_bytes());

        match value {
            Value::Text(text) => {
                bytes.extend_from_slice(&8u32.to_le_bytes());
                bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
            }
            Value::U32(number) => {
                bytes.extend_from_slice(&4u32.to_le_bytes());
                bytes.extend_from_slice(&number.to_le_bytes());
            }
            Value::U64(number) => {
                bytes.extend_from_slice(&10u32.to_le_bytes());
                bytes.extend_from_slice(&number.to_le_bytes());
            }
            Value::Bool(flag) => {
                bytes.extend_from_slice(&7u32.to_le_bytes());
                bytes.push(u8::from(flag));
            }
            Value::F32(number) => {
                bytes.extend_from_slice(&6u32.to_le_bytes());
                bytes.extend_from_slice(&number.to_le_bytes());
            }
            Value::TextArray(items) => {
                bytes.extend_from_slice(&9u32.to_le_bytes());
                bytes.extend_from_slice(&8u32.to_le_bytes());
                bytes.extend_from_slice(&(items.len() as u64).to_le_bytes());

                for item in items {
                    bytes.extend_from_slice(&(item.len() as u64).to_le_bytes());
                    bytes.extend_from_slice(item.as_bytes());
                }
            }
            Value::U32Array(items) => {
                bytes.extend_from_slice(&9u32.to_le_bytes());
                bytes.extend_from_slice(&4u32.to_le_bytes());
                bytes.extend_from_slice(&(items.len() as u64).to_le_bytes());

                for item in items {
                    bytes.extend_from_slice(&item.to_le_bytes());
                }
            }
        }
    }

    fn metadata_block(entries: &[(&str, Value<'_>)]) -> Vec<u8> {
        let mut bytes = Vec::new();

        for (key, value) in entries {
            push_kv(&mut bytes, key, value.clone());
        }

        bytes
    }

    fn gguf_with_metadata(entries: &[(&str, Value<'_>)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&291u64.to_le_bytes());
        bytes.extend_from_slice(&(entries.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&metadata_block(entries));
        bytes.resize((MIN_PLAUSIBLE_BYTES + 1) as usize, 0);
        bytes
    }

    #[cfg(windows)]
    #[test]
    fn read_guard_denies_write_and_delete_until_released() {
        let path = temp_file("locked-model.gguf", b"guarded model bytes");
        let locked = LockedFile::open(&path).expect("open write/delete-denying guard");

        assert!(
            OpenOptions::new().write(true).open(&path).is_err(),
            "Windows share mode must deny a writer while the runtime guard is alive"
        );
        assert!(
            std::fs::remove_file(&path).is_err(),
            "Windows share mode must deny deletion while the runtime guard is alive"
        );

        drop(locked);
        assert!(OpenOptions::new().write(true).open(&path).is_ok());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_a_missing_file() {
        let error = inspect("C:/definitely/not/here.gguf", 32768, None).unwrap_err();

        assert_eq!(error.code, "invalid_model");
    }

    #[test]
    fn rejects_a_non_gguf_extension() {
        let path = temp_file("model.bin", &gguf_header(3, 4));

        let error = inspect(path.to_str().unwrap(), 32768, None).unwrap_err();

        assert_eq!(error.code, "invalid_model");
    }

    #[test]
    fn rejects_wrong_magic_bytes() {
        let mut bytes = gguf_header(3, 4);
        bytes[0..4].copy_from_slice(b"XXXX");
        let path = temp_file("wrong-magic.gguf", &bytes);

        let error = inspect(path.to_str().unwrap(), 32768, None).unwrap_err();

        assert!(error.message.contains("GGUF header"));
    }

    #[test]
    fn rejects_an_unsupported_version() {
        let path = temp_file("v9.gguf", &gguf_header(9, 4));

        let error = inspect(path.to_str().unwrap(), 32768, None).unwrap_err();

        assert!(error.message.contains("version 9"));
    }

    #[test]
    fn accepts_a_well_formed_header_and_estimates_memory() {
        let path = temp_file("good.gguf", &gguf_header(3, 291));

        let info = inspect(path.to_str().unwrap(), 32768, Some(32.0)).expect("valid");

        assert_eq!(info.gguf_version, 3);
        assert_eq!(info.tensor_count, 291);
        assert!(info.estimated_min_ram_gib > 1.0);
        assert_eq!(info.fits_available_ram, Some(true));
        assert!(info.warnings.is_empty());
    }

    #[test]
    fn warns_when_the_model_exceeds_available_memory() {
        let path = temp_file("big.gguf", &gguf_header(3, 100));

        let info = inspect(path.to_str().unwrap(), 32768, Some(2.0)).expect("valid");

        assert_eq!(info.fits_available_ram, Some(false));
        assert_eq!(info.warnings.len(), 1);
    }

    #[test]
    fn reads_architecture_name_and_context() {
        let block = metadata_block(&[
            ("general.architecture", Value::Text("qwen2")),
            ("general.name", Value::Text("Qwen2.5-Coder-7B-Instruct")),
            ("qwen2.context_length", Value::U32(32_768)),
            ("qwen2.embedding_length", Value::U32(3_584)),
            ("qwen2.attention.head_count", Value::U32(28)),
        ]);

        let metadata = read_metadata(&mut block.as_slice(), 5).expect("metadata");

        assert_eq!(metadata.architecture.as_deref(), Some("qwen2"));
        assert_eq!(metadata.name.as_deref(), Some("Qwen2.5-Coder-7B-Instruct"));
        assert_eq!(metadata.context_length, Some(32_768));
    }

    #[test]
    fn accepts_a_64_bit_context_length() {
        let block = metadata_block(&[("qwen2.context_length", Value::U64(131_072))]);

        let metadata = read_metadata(&mut block.as_slice(), 1).expect("metadata");

        assert_eq!(metadata.context_length, Some(131_072));
    }

    #[test]
    fn skips_tokenizer_arrays_and_scalars_by_length() {
        let tokens = ["hello", "world", "<|im_end|>"];
        let types = [1u32, 2, 3];
        let block = metadata_block(&[
            ("tokenizer.ggml.tokens", Value::TextArray(&tokens)),
            ("tokenizer.ggml.token_type", Value::U32Array(&types)),
            ("tokenizer.ggml.add_bos_token", Value::Bool(false)),
            (
                "qwen2.attention.layer_norm_rms_epsilon",
                Value::F32(0.000_001),
            ),
            ("qwen2.context_length", Value::U32(16_384)),
        ]);

        let metadata = read_metadata(&mut block.as_slice(), 5).expect("metadata");

        assert_eq!(metadata.context_length, Some(16_384));
    }

    #[test]
    fn reports_a_truncated_metadata_table() {
        let block = metadata_block(&[
            ("general.architecture", Value::Text("qwen2")),
            ("qwen2.context_length", Value::U32(32_768)),
        ]);

        let cut = &block[..block.len() - 6];
        let error = read_metadata(&mut { cut }, 2).unwrap_err();

        assert!(error.contains("ended inside its metadata table"), "{error}");
    }

    #[test]
    fn rejects_an_absurd_array_count() {
        let mut block = Vec::new();
        block.extend_from_slice(&4u64.to_le_bytes());
        block.extend_from_slice(b"arr!");
        block.extend_from_slice(&9u32.to_le_bytes());
        block.extend_from_slice(&8u32.to_le_bytes());
        block.extend_from_slice(&u64::MAX.to_le_bytes());

        let error = read_metadata(&mut block.as_slice(), 1).unwrap_err();

        assert!(error.contains("more elements"), "{error}");
    }

    #[test]
    fn rejects_an_unknown_value_type() {
        let mut block = Vec::new();
        block.extend_from_slice(&3u64.to_le_bytes());
        block.extend_from_slice(b"odd");
        block.extend_from_slice(&99u32.to_le_bytes());

        let error = read_metadata(&mut block.as_slice(), 1).unwrap_err();

        assert!(error.contains("not supported"), "{error}");
    }

    #[test]
    fn rejects_a_metadata_table_beyond_the_budget() {
        let error = read_metadata(&mut std::io::empty(), MAX_METADATA_ENTRIES + 1).unwrap_err();

        assert!(error.contains("metadata entries"), "{error}");
    }

    #[test]
    fn exposes_metadata_on_a_whole_file_and_warns_about_context() {
        let bytes = gguf_with_metadata(&[
            ("general.architecture", Value::Text("qwen2")),
            ("general.name", Value::Text("Qwen2.5-Coder-7B-Instruct")),
            ("qwen2.context_length", Value::U32(8_192)),
        ]);
        let path = temp_file("with-metadata.gguf", &bytes);

        let info = inspect(path.to_str().unwrap(), 32_768, None).expect("valid");

        assert_eq!(info.architecture.as_deref(), Some("qwen2"));
        assert_eq!(info.name.as_deref(), Some("Qwen2.5-Coder-7B-Instruct"));
        assert_eq!(info.context_length, Some(8_192));
        assert!(info
            .warnings
            .iter()
            .any(|warning| warning.contains("maximum context of 8192")));
    }

    #[test]
    fn warns_but_accepts_a_file_with_unreadable_metadata() {
        let mut bytes = gguf_header(3, 291);
        bytes[16..24].copy_from_slice(&1u64.to_le_bytes());
        // A key length of u64::MAX cannot be honest: the table is corrupt.
        bytes[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
        let path = temp_file("bad-metadata.gguf", &bytes);

        let info = inspect(path.to_str().unwrap(), 32_768, None).expect("header still valid");

        assert!(info.architecture.is_none());
        assert!(info.context_length.is_none());
        assert!(info
            .warnings
            .iter()
            .any(|warning| warning.contains("metadata could not be read")));
    }

    #[test]
    fn detects_a_windows_executable() {
        let path = temp_file("llama-server.exe", b"MZ\x90\x00payload");

        #[cfg(windows)]
        assert!(looks_like_executable(path.to_str().unwrap()).is_ok());

        // A PE file is the wrong binary on Linux; the check must say so rather
        // than accept it and fail later inside the OS loader.
        #[cfg(not(windows))]
        assert!(looks_like_executable(path.to_str().unwrap()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn detects_an_elf_executable_on_unix() {
        let path = temp_file("llama-server", b"\x7fELF\x02\x01\x01payload");

        assert!(looks_like_executable(path.to_str().unwrap()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn detects_a_script_launcher() {
        let path = temp_file("llama-server.sh", b"#!/bin/sh\nexec llama-server $@\n");

        assert!(looks_like_executable(path.to_str().unwrap()).is_ok());
    }

    #[test]
    fn rejects_a_non_executable_runtime() {
        let path = temp_file("readme.exe", b"hello");

        assert!(looks_like_executable(path.to_str().unwrap()).is_err());
    }
}
