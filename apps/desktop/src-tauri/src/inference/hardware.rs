use serde::Serialize;

const GIB_BYTES: f64 = 1024.0 * 1024.0 * 1024.0;

/// Memory the engine can see. Every field is optional: an unsupported platform
/// reports "unknown" rather than a guess.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub total_ram_gib: Option<f64>,
    pub available_ram_gib: Option<f64>,
    pub platform: &'static str,
}

/// Free space at one filesystem location.
///
/// `free_bytes: None` means detection is unavailable on this platform or the
/// path could not be inspected — never a guess. This is what the readiness
/// logic reads; it never shells out to measure a disk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpace {
    /// Free bytes available to the current user.
    pub free_bytes: Option<u64>,
    /// Total bytes on the filesystem, when known.
    pub total_bytes: Option<u64>,
    /// Why the value is `None`, when it is.
    pub unavailable_reason: Option<&'static str>,
}

impl DiskSpace {
    fn unknown(reason: &'static str) -> Self {
        Self {
            free_bytes: None,
            total_bytes: None,
            unavailable_reason: Some(reason),
        }
    }
}

/// Free space at `path`, or an explicit "unknown".
///
/// A path that does not exist yet is inspected through its nearest existing
/// parent, because the model folder is created lazily. Nothing is created,
/// downloaded, or loaded here.
pub fn disk_space(path: &std::path::Path) -> DiskSpace {
    let existing = nearest_existing(path);

    let Some(existing) = existing else {
        return DiskSpace::unknown("The folder does not exist yet and has no existing parent.");
    };

    match free_space_bytes(&existing) {
        Some((free, total)) => DiskSpace {
            free_bytes: Some(free),
            total_bytes: Some(total),
            unavailable_reason: None,
        },
        None => DiskSpace::unknown("Free space could not be read on this platform."),
    }
}

/// Walks up until a path exists. Bounded so a pathological path cannot loop.
fn nearest_existing(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = path.to_path_buf();

    for _ in 0..64 {
        if current.exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }

    None
}

pub fn system_info() -> SystemInfo {
    let (total, available) = memory_bytes();

    SystemInfo {
        total_ram_gib: total.map(|bytes| round1(bytes as f64 / GIB_BYTES)),
        available_ram_gib: available.map(|bytes| round1(bytes as f64 / GIB_BYTES)),
        platform: std::env::consts::OS,
    }
}

/// Total and currently available physical memory in bytes, when the current
/// platform exposes those readings without elevated permissions.
pub fn memory_bytes() -> (Option<u64>, Option<u64>) {
    platform_memory_bytes()
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Total and available physical memory, in bytes.
#[cfg(windows)]
fn platform_memory_bytes() -> (Option<u64>, Option<u64>) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    // SAFETY: `status` is a plain C struct owned by this function, its length
    // field is set before the call, and the pointer is valid for the call.
    unsafe {
        let mut status = std::mem::zeroed::<MEMORYSTATUSEX>();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;

        if GlobalMemoryStatusEx(&mut status) == 0 {
            return (None, None);
        }

        (Some(status.ullTotalPhys), Some(status.ullAvailPhys))
    }
}

#[cfg(target_os = "linux")]
fn platform_memory_bytes() -> (Option<u64>, Option<u64>) {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();

    (
        meminfo_bytes(&text, "MemTotal:"),
        meminfo_bytes(&text, "MemAvailable:"),
    )
}

#[cfg(target_os = "linux")]
fn meminfo_bytes(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|line| line.strip_prefix(key))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|kib| kib.saturating_mul(1024))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_memory_bytes() -> (Option<u64>, Option<u64>) {
    (None, None)
}

// ---------------------------------------------------------------------------
// Free disk space
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn free_space_bytes(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // `struct statvfs` on 64-bit Linux, declared locally so no new crate is
    // added. Field order and widths are fixed by the glibc ABI on LP64.
    #[repr(C)]
    struct StatVfs {
        f_bsize: u64,
        f_frsize: u64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_favail: u64,
        f_fsid: u64,
        f_flag: u64,
        f_namemax: u64,
        f_spare: [i32; 6],
    }

    extern "C" {
        fn statvfs(path: *const std::os::raw::c_char, buf: *mut StatVfs) -> std::os::raw::c_int;
    }

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;

    // SAFETY: `stats` is a correctly sized plain struct owned by this function,
    // and `c_path` is a valid NUL-terminated string for the duration of the call.
    let mut stats: StatVfs = unsafe { std::mem::zeroed() };

    let result = unsafe { statvfs(c_path.as_ptr(), &mut stats) };

    if result != 0 {
        return None;
    }

    let block = if stats.f_frsize > 0 {
        stats.f_frsize
    } else {
        stats.f_bsize
    };

    if block == 0 {
        return None;
    }

    Some((
        stats.f_bavail.saturating_mul(block),
        stats.f_blocks.saturating_mul(block),
    ))
}

#[cfg(windows)]
fn free_space_bytes(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    let mut free_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;

    // SAFETY: the path is NUL-terminated and each out-pointer refers to a live
    // u64 owned by this function for the duration of the call.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_to_caller as *mut u64,
            &mut total as *mut u64,
            &mut total_free as *mut u64,
        )
    };

    if ok == 0 {
        return None;
    }

    Some((free_to_caller, total))
}

#[cfg(not(any(windows, all(target_os = "linux", target_pointer_width = "64"))))]
fn free_space_bytes(_path: &std::path::Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_platform_and_plausible_numbers() {
        let info = system_info();

        assert!(!info.platform.is_empty());

        if let Some(total) = info.total_ram_gib {
            assert!(total > 0.0, "total memory should be positive");
        }
    }

    #[test]
    fn disk_space_never_panics_on_a_missing_path() {
        let space = disk_space(std::path::Path::new("/definitely/not/a/real/path/here"));

        // Either it found a parent and measured it, or it says unknown. Both
        // are acceptable; a panic or a fabricated number is not.
        assert!(space.free_bytes.is_some() || space.unavailable_reason.is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn disk_space_reports_real_values_for_the_temp_directory() {
        let space = disk_space(&std::env::temp_dir());

        let free = space.free_bytes.expect("free space on /tmp");
        let total = space.total_bytes.expect("total space on /tmp");

        assert!(free > 0, "free space should be positive");
        assert!(total >= free, "total must be at least free");
        assert!(space.unavailable_reason.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nearest_existing_walks_up_to_a_real_parent() {
        let resolved = nearest_existing(std::path::Path::new("/tmp/nexora-not-created/child"));

        assert_eq!(resolved, Some(std::path::PathBuf::from("/tmp")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_meminfo_lines() {
        let sample = "MemTotal:       16384000 kB\nMemAvailable:    8192000 kB\n";

        assert_eq!(meminfo_kib(sample, "MemTotal:"), Some(16_384_000.0));
        assert_eq!(meminfo_kib(sample, "MemAvailable:"), Some(8_192_000.0));
        assert_eq!(meminfo_kib(sample, "Missing:"), None);
    }
}
