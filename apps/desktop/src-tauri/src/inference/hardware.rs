use serde::Serialize;

// Only the Windows implementation converts raw bytes with it; the Linux path
// derives GiB from /proc/meminfo directly.
#[cfg(windows)]
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Memory the engine can see. Every field is optional: an unsupported platform
/// reports "unknown" rather than a guess.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub total_ram_gib: Option<f64>,
    pub available_ram_gib: Option<f64>,
    pub platform: &'static str,
}

pub fn system_info() -> SystemInfo {
    let (total, available) = memory_gib();

    SystemInfo {
        total_ram_gib: total.map(round1),
        available_ram_gib: available.map(round1),
        platform: std::env::consts::OS,
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Total and available physical memory, in GiB.
#[cfg(windows)]
fn memory_gib() -> (Option<f64>, Option<f64>) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    // SAFETY: `status` is a plain C struct owned by this function, its length
    // field is set before the call, and the pointer is valid for the call.
    unsafe {
        let mut status = std::mem::zeroed::<MEMORYSTATUSEX>();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;

        if GlobalMemoryStatusEx(&mut status) == 0 {
            return (None, None);
        }

        (
            Some(status.ullTotalPhys as f64 / GIB),
            Some(status.ullAvailPhys as f64 / GIB),
        )
    }
}

#[cfg(target_os = "linux")]
fn memory_gib() -> (Option<f64>, Option<f64>) {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();

    (
        meminfo_kib(&text, "MemTotal:").map(to_gib),
        meminfo_kib(&text, "MemAvailable:").map(to_gib),
    )
}

#[cfg(target_os = "linux")]
fn meminfo_kib(text: &str, key: &str) -> Option<f64> {
    text.lines()
        .find_map(|line| line.strip_prefix(key))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
}

#[cfg(target_os = "linux")]
fn to_gib(kib: f64) -> f64 {
    kib / (1024.0 * 1024.0)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn memory_gib() -> (Option<f64>, Option<f64>) {
    (None, None)
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

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_meminfo_lines() {
        let sample = "MemTotal:       16384000 kB\nMemAvailable:    8192000 kB\n";

        assert_eq!(meminfo_kib(sample, "MemTotal:"), Some(16_384_000.0));
        assert_eq!(meminfo_kib(sample, "MemAvailable:"), Some(8_192_000.0));
        assert_eq!(meminfo_kib(sample, "Missing:"), None);
    }
}
