use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;

use super::error::{InferenceError, InferenceResult};

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

    if !file_path.is_file() {
        return Err(InferenceError::invalid_model(format!(
            "No file exists at {trimmed}."
        )));
    }

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

    let mut file = File::open(file_path).map_err(|error| {
        InferenceError::invalid_model(format!("The model file could not be opened: {error}"))
    })?;

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
    file.read_exact(&mut header).map_err(|_| {
        InferenceError::invalid_model("The file ended before its GGUF header did.")
    })?;

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

    let size_gib = size_bytes as f64 / GIB;
    let mut warnings = Vec::new();

    if tensor_count == 0 {
        warnings.push("The header lists no tensors; the file may be truncated.".to_string());
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
        estimated_min_ram_gib: round2(estimated_min_ram_gib),
        fits_available_ram,
        warnings,
    })
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Reads a bounded prefix of the runtime executable so a wrong file is caught
/// before it is ever launched.
pub fn looks_like_executable(path: &str) -> InferenceResult<()> {
    let file_path = Path::new(path.trim());

    if !file_path.is_file() {
        return Err(InferenceError::not_configured(format!(
            "No runtime executable exists at {path}."
        )));
    }

    let mut file = File::open(file_path)?;
    let mut magic = [0u8; 4];
    let read = read_magic(&mut file, &mut magic)?;

    if read < 2 {
        return Err(InferenceError::invalid_model(
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
    let runs_here =
        (is_windows_binary && cfg!(windows)) || (is_script && cfg!(unix)) || is_elf;

    if !runs_here {
        return Err(InferenceError::invalid_model(if cfg!(windows) {
            "That file is not a Windows executable. Choose llama-server.exe."
        } else {
            "That file is not an executable for this platform. Choose the llama-server binary."
        }));
    }

    // Reject a directory masquerading as a file and confirm we can seek.
    file.seek(SeekFrom::Start(0))
        .map_err(|error| InferenceError::invalid_model(error.to_string()))?;

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
        bytes.extend_from_slice(&16u64.to_le_bytes());
        bytes.resize((MIN_PLAUSIBLE_BYTES + 1) as usize, 0);
        bytes
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
