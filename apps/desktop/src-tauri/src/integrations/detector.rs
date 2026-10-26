use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::commands::{
    DetectedInstallation, DetectionConfidence, DetectionMethod, InstallationDetectionResult,
};

const MAX_ROOT_ENTRIES: usize = 128;
const MAX_FOUND_CANDIDATES: usize = 16;
const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PE_HEADER_OFFSET: u64 = 1024 * 1024;
const VALIDATION_NOTE: &str = "Candidate only: canonical path is beneath a bounded Blender Foundation directory, basename is blender.exe, and the file has a structurally valid supported-architecture Windows PE header. Publisher identity, Blender product metadata, version, and compatibility are not verified.";

#[derive(Clone, Debug)]
struct SearchRoot {
    label: &'static str,
    path: PathBuf,
}

#[derive(Debug)]
enum RootFailure {
    AccessDenied,
    SearchLimit,
    Io,
}

#[derive(Debug, PartialEq, Eq)]
enum CandidateFailure {
    AccessDenied,
    Rejected,
    Io,
}

/// The only actual detector in this module is Windows-specific. It checks a
/// short allowlist of standard install roots and does not launch candidates.
pub fn detect_blender_installations() -> InstallationDetectionResult {
    #[cfg(not(windows))]
    {
        InstallationDetectionResult::Unsupported {
            reason: "Blender installation detection is Windows-only; this build did not inspect local files.".to_string(),
        }
    }

    #[cfg(windows)]
    {
        let (roots, unavailable_roots) = standard_windows_roots();
        scan_standard_roots(&roots, unavailable_roots)
    }
}

#[cfg(windows)]
fn standard_windows_roots() -> (Vec<SearchRoot>, Vec<String>) {
    let sources = [
        (
            "program_files_blender_foundation",
            std::env::var_os("ProgramFiles")
                .map(|value| PathBuf::from(value).join("Blender Foundation")),
        ),
        (
            "program_files_x86_blender_foundation",
            std::env::var_os("ProgramFiles(x86)")
                .map(|value| PathBuf::from(value).join("Blender Foundation")),
        ),
        (
            "local_app_data_blender_foundation",
            std::env::var_os("LOCALAPPDATA").map(|value| {
                PathBuf::from(value)
                    .join("Programs")
                    .join("Blender Foundation")
            }),
        ),
    ];

    let mut roots = Vec::new();
    let mut unavailable = Vec::new();
    let mut seen = HashSet::new();

    for (label, path) in sources {
        match path {
            Some(path) if is_supported_local_root(&path) => {
                let key = lexical_path_key(&path);
                if seen.insert(key) {
                    roots.push(SearchRoot { label, path });
                }
            }
            Some(_) | None => unavailable.push(label.to_string()),
        }
    }

    (roots, unavailable)
}

#[cfg(windows)]
fn is_supported_local_root(path: &Path) -> bool {
    use std::path::{Component, Prefix};

    path.is_absolute()
        && matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
}

#[cfg(windows)]
fn scan_standard_roots(
    roots: &[SearchRoot],
    mut incomplete_locations: Vec<String>,
) -> InstallationDetectionResult {
    if roots.is_empty() {
        return InstallationDetectionResult::Error {
            code: "search_locations_unavailable".to_string(),
            message: "Windows did not provide any supported Blender installation roots."
                .to_string(),
            incomplete_locations,
        };
    }

    let mut checked_locations = Vec::new();
    let mut candidate_paths = Vec::new();
    let mut rejected_candidates = 0_u32;
    let mut search_limit_hit = false;
    let mut io_failure = false;
    let mut access_denied = false;

    for root in roots {
        checked_locations.push(root.label.to_string());
        let canonical_root = match fs::canonicalize(&root.path) {
            Ok(path) if is_supported_local_root(&path) => path,
            Ok(_) => {
                incomplete_locations.push(root.label.to_string());
                io_failure = true;
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                incomplete_locations.push(root.label.to_string());
                access_denied = true;
                continue;
            }
            Err(_) => {
                incomplete_locations.push(root.label.to_string());
                io_failure = true;
                continue;
            }
        };

        let metadata = match fs::metadata(&canonical_root) {
            Ok(metadata) if metadata.is_dir() => metadata,
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                incomplete_locations.push(root.label.to_string());
                access_denied = true;
                continue;
            }
            Err(_) => {
                incomplete_locations.push(root.label.to_string());
                io_failure = true;
                continue;
            }
        };
        drop(metadata);

        let directories = match immediate_blender_directories(&canonical_root) {
            Ok(directories) => directories,
            Err(RootFailure::AccessDenied) => {
                incomplete_locations.push(root.label.to_string());
                access_denied = true;
                Vec::new()
            }
            Err(RootFailure::SearchLimit) => {
                incomplete_locations.push(root.label.to_string());
                search_limit_hit = true;
                Vec::new()
            }
            Err(RootFailure::Io) => {
                incomplete_locations.push(root.label.to_string());
                io_failure = true;
                Vec::new()
            }
        };

        candidate_paths.push(canonical_root.join("blender.exe"));
        candidate_paths.extend(
            directories
                .into_iter()
                .map(|directory| directory.join("blender.exe")),
        );
    }

    let mut installations = Vec::new();
    let mut seen_installation_paths = HashSet::new();
    for candidate in deduplicate_candidate_paths(candidate_paths) {
        let canonical_candidate = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                if let Some(label) = roots.iter().find_map(|root| {
                    fs::canonicalize(&root.path)
                        .ok()
                        .filter(|canonical_root| candidate.starts_with(canonical_root))
                        .map(|_| root.label)
                }) {
                    incomplete_locations.push(label.to_string());
                }
                access_denied = true;
                continue;
            }
            Err(_) => {
                if let Some(label) = roots.iter().find_map(|root| {
                    fs::canonicalize(&root.path)
                        .ok()
                        .filter(|canonical_root| candidate.starts_with(canonical_root))
                        .map(|_| root.label)
                }) {
                    incomplete_locations.push(label.to_string());
                }
                io_failure = true;
                continue;
            }
        };

        let matching_root = roots.iter().find_map(|root| {
            fs::canonicalize(&root.path)
                .ok()
                .filter(|canonical_root| canonical_candidate.starts_with(canonical_root))
                .map(|_| root.label)
        });
        if matching_root.is_none() || !has_blender_executable_name(&canonical_candidate) {
            rejected_candidates = rejected_candidates.saturating_add(1);
            continue;
        }
        if !seen_installation_paths.insert(lexical_path_key(&canonical_candidate)) {
            continue;
        }

        match validate_pe_candidate(&canonical_candidate) {
            Ok(()) => installations.push(DetectedInstallation {
                executable_path: canonical_candidate.to_string_lossy().into_owned(),
                version: None,
                method: DetectionMethod::StandardDirectory,
                confidence: DetectionConfidence::CandidateOnly,
                validation: VALIDATION_NOTE.to_string(),
            }),
            Err(CandidateFailure::AccessDenied) => {
                if let Some(label) = matching_root {
                    incomplete_locations.push(label.to_string());
                }
                access_denied = true;
            }
            Err(CandidateFailure::Rejected) => {
                rejected_candidates = rejected_candidates.saturating_add(1);
            }
            Err(CandidateFailure::Io) => {
                if let Some(label) = matching_root {
                    incomplete_locations.push(label.to_string());
                }
                io_failure = true;
            }
        }
    }

    incomplete_locations.sort();
    incomplete_locations.dedup();

    if !installations.is_empty() {
        installations.sort_by(|left, right| {
            left.executable_path
                .to_ascii_lowercase()
                .cmp(&right.executable_path.to_ascii_lowercase())
        });
        if installations.len() > MAX_FOUND_CANDIDATES {
            return InstallationDetectionResult::Error {
                code: "candidate_limit_exceeded".to_string(),
                message:
                    "More Blender candidates were found than this bounded detector can report."
                        .to_string(),
                incomplete_locations,
            };
        }
        return InstallationDetectionResult::Found {
            installations,
            incomplete_locations,
            rejected_candidates,
        };
    }

    if search_limit_hit || io_failure || access_denied || !incomplete_locations.is_empty() {
        if access_denied
            && !io_failure
            && !search_limit_hit
            && checked_locations
                .iter()
                .all(|location| incomplete_locations.contains(location))
        {
            return InstallationDetectionResult::AccessDenied {
                reason: "Access to all configured Blender search locations was denied.".to_string(),
                incomplete_locations,
            };
        }

        let code = if access_denied && !io_failure && !search_limit_hit {
            "access_denied"
        } else if search_limit_hit {
            "search_limit_exceeded"
        } else {
            "incomplete_search"
        };
        return InstallationDetectionResult::Error {
            code: code.to_string(),
            message: "The bounded Blender search could not check all supported locations."
                .to_string(),
            incomplete_locations,
        };
    }

    InstallationDetectionResult::NotFound {
        checked_locations,
        rejected_candidates,
    }
}

fn immediate_blender_directories(root: &Path) -> Result<Vec<PathBuf>, RootFailure> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Err(RootFailure::AccessDenied)
        }
        Err(_) => return Err(RootFailure::Io),
    };

    let mut directories = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ROOT_ENTRIES {
            return Err(RootFailure::SearchLimit);
        }
        let entry = entry.map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                RootFailure::AccessDenied
            } else {
                RootFailure::Io
            }
        })?;
        let file_type = entry.file_type().map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                RootFailure::AccessDenied
            } else {
                RootFailure::Io
            }
        })?;
        if file_type.is_dir()
            && !file_type.is_symlink()
            && is_blender_directory_name(&entry.file_name())
        {
            directories.push(entry.path());
        }
    }

    Ok(directories)
}

fn is_blender_directory_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_lowercase();
    name == "blender" || name.starts_with("blender ")
}

fn has_blender_executable_name(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().eq_ignore_ascii_case("blender.exe"))
        .unwrap_or(false)
}

fn deduplicate_candidate_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(lexical_path_key(path)))
        .collect()
}

fn lexical_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn validate_pe_candidate(path: &Path) -> Result<(), CandidateFailure> {
    if !has_blender_executable_name(path) {
        return Err(CandidateFailure::Rejected);
    }

    let metadata = fs::metadata(path).map_err(map_candidate_io_error)?;
    if !metadata.is_file() || metadata.len() > MAX_EXECUTABLE_BYTES {
        return Err(CandidateFailure::Rejected);
    }

    let mut file = File::open(path).map_err(map_candidate_io_error)?;
    validate_pe_headers(&mut file, metadata.len()).map_err(|error| match error {
        PeValidationError::AccessDenied => CandidateFailure::AccessDenied,
        PeValidationError::Invalid => CandidateFailure::Rejected,
        PeValidationError::Io => CandidateFailure::Io,
    })
}

fn map_candidate_io_error(error: io::Error) -> CandidateFailure {
    match error.kind() {
        io::ErrorKind::PermissionDenied => CandidateFailure::AccessDenied,
        io::ErrorKind::NotFound => CandidateFailure::Rejected,
        _ => CandidateFailure::Io,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PeValidationError {
    AccessDenied,
    Invalid,
    Io,
}

fn validate_pe_headers(file: &mut File, length: u64) -> Result<(), PeValidationError> {
    if length < 64 || length > MAX_EXECUTABLE_BYTES {
        return Err(PeValidationError::Invalid);
    }

    let mut dos_header = [0_u8; 64];
    file.read_exact(&mut dos_header).map_err(map_pe_io_error)?;
    if &dos_header[0..2] != b"MZ" {
        return Err(PeValidationError::Invalid);
    }

    let pe_offset = u32::from_le_bytes([
        dos_header[0x3c],
        dos_header[0x3d],
        dos_header[0x3e],
        dos_header[0x3f],
    ]) as u64;
    if !(64..=MAX_PE_HEADER_OFFSET).contains(&pe_offset)
        || pe_offset
            .checked_add(26)
            .filter(|end| *end <= length)
            .is_none()
    {
        return Err(PeValidationError::Invalid);
    }

    file.seek(SeekFrom::Start(pe_offset))
        .map_err(map_pe_io_error)?;
    let mut pe_and_coff = [0_u8; 24];
    file.read_exact(&mut pe_and_coff).map_err(map_pe_io_error)?;
    if &pe_and_coff[0..4] != b"PE\0\0" {
        return Err(PeValidationError::Invalid);
    }

    let machine = u16::from_le_bytes([pe_and_coff[4], pe_and_coff[5]]);
    let section_count = u16::from_le_bytes([pe_and_coff[6], pe_and_coff[7]]);
    let optional_header_size = u16::from_le_bytes([pe_and_coff[20], pe_and_coff[21]]) as u64;
    if section_count == 0
        || optional_header_size < 2
        || pe_offset
            .checked_add(24 + optional_header_size)
            .filter(|end| *end <= length)
            .is_none()
    {
        return Err(PeValidationError::Invalid);
    }

    let mut optional_magic = [0_u8; 2];
    file.read_exact(&mut optional_magic)
        .map_err(map_pe_io_error)?;
    let magic = u16::from_le_bytes(optional_magic);
    let architecture_matches = matches!(
        (machine, magic),
        (0x014c, 0x010b) | (0x8664, 0x020b) | (0xaa64, 0x020b)
    );
    if !architecture_matches {
        return Err(PeValidationError::Invalid);
    }

    Ok(())
}

fn map_pe_io_error(error: io::Error) -> PeValidationError {
    match error.kind() {
        io::ErrorKind::PermissionDenied => PeValidationError::AccessDenied,
        io::ErrorKind::UnexpectedEof | io::ErrorKind::NotFound => PeValidationError::Invalid,
        _ => PeValidationError::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_minimal_pe(path: &Path, machine: u16, optional_magic: u16) {
        let mut bytes = vec![0_u8; 512];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&1_u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&2_u16.to_le_bytes());
        bytes[0x98..0x9a].copy_from_slice(&optional_magic.to_le_bytes());
        File::create(path).unwrap().write_all(&bytes).unwrap();
    }

    #[test]
    fn accepts_supported_pe_header_shapes_without_claiming_blender_identity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("blender.exe");
        write_minimal_pe(&path, 0x8664, 0x020b);

        assert_eq!(validate_pe_candidate(&path), Ok(()));
    }

    #[test]
    fn rejects_wrong_names_non_pe_files_and_architecture_mismatches() {
        let temp = tempfile::tempdir().unwrap();
        let other_name = temp.path().join("other.exe");
        write_minimal_pe(&other_name, 0x8664, 0x020b);
        assert!(matches!(
            validate_pe_candidate(&other_name),
            Err(CandidateFailure::Rejected)
        ));

        let invalid = temp.path().join("blender.exe");
        File::create(&invalid)
            .unwrap()
            .write_all(b"not a PE image")
            .unwrap();
        assert!(matches!(
            validate_pe_candidate(&invalid),
            Err(CandidateFailure::Rejected)
        ));

        write_minimal_pe(&invalid, 0x8664, 0x010b);
        assert!(matches!(
            validate_pe_candidate(&invalid),
            Err(CandidateFailure::Rejected)
        ));
    }

    #[test]
    fn enumerates_only_immediate_blender_named_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Blender Foundation");
        fs::create_dir_all(root.join("Blender 4.2").join("nested").join("Blender 3.6")).unwrap();
        fs::create_dir_all(root.join("Blender")).unwrap();
        fs::create_dir_all(root.join("Other App")).unwrap();
        File::create(root.join("Blender 4.0.exe")).unwrap();

        let mut directories = immediate_blender_directories(&root).unwrap();
        directories.sort();
        assert_eq!(
            directories,
            vec![root.join("Blender"), root.join("Blender 4.2")]
        );
    }

    #[test]
    fn stops_after_the_configured_immediate_entry_bound() {
        let temp = tempfile::tempdir().unwrap();
        for index in 0..=MAX_ROOT_ENTRIES {
            fs::create_dir(temp.path().join(format!("Blender {index}"))).unwrap();
        }

        assert!(matches!(
            immediate_blender_directories(temp.path()),
            Err(RootFailure::SearchLimit)
        ));
    }

    #[test]
    fn deduplicates_paths_without_following_or_executing_them() {
        let paths = vec![
            PathBuf::from(r"C:\Program Files\Blender Foundation\Blender 4.2\blender.exe"),
            PathBuf::from(r"c:/program files/blender foundation/blender 4.2/blender.exe"),
        ];
        assert_eq!(deduplicate_candidate_paths(paths).len(), 1);
    }

    #[cfg(not(windows))]
    #[test]
    fn reports_unsupported_platform_without_scanning() {
        assert!(matches!(
            detect_blender_installations(),
            InstallationDetectionResult::Unsupported { .. }
        ));
    }
}
