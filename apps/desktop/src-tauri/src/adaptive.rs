//! Read-only, best-effort hardware capability snapshot for future local media
//! workloads. No runtime is started and no memory is reserved or files scanned.

use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::inference::hardware;

const GIB: u64 = 1024 * 1024 * 1024;

/// Internal resource screens. These are deliberately conservative starting
/// points, not model-fit guarantees; later model-specific planning can tune them.
struct ProfileThresholds {
    total_ram: u64,
    available_ram: u64,
    dedicated_vram: u64,
    storage_free: u64,
}

const BALANCED_THRESHOLDS: ProfileThresholds = ProfileThresholds {
    total_ram: 16 * GIB,
    available_ram: 8 * GIB,
    dedicated_vram: 8 * GIB,
    storage_free: 80 * GIB,
};
const QUALITY_THRESHOLDS: ProfileThresholds = ProfileThresholds {
    total_ram: 32 * GIB,
    available_ram: 16 * GIB,
    dedicated_vram: 16 * GIB,
    storage_free: 200 * GIB,
};
const IMAGE_SCREENING_FLOORS: [u64; 3] = [8 * GIB, 4 * GIB, 20 * GIB];
const VIDEO_SCREENING_FLOORS: [u64; 3] = [16 * GIB, 8 * GIB, 80 * GIB];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectionConfidence {
    Confirmed,
    Estimated,
    Unavailable,
    Unknown,
}

/// A missing reading is represented explicitly, never as a fabricated zero.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection<T> {
    pub value: Option<T>,
    pub confidence: DetectionConfidence,
    pub detail: Option<String>,
}

impl<T> Detection<T> {
    fn confirmed(value: T, detail: impl Into<String>) -> Self {
        Self { value: Some(value), confidence: DetectionConfidence::Confirmed, detail: Some(detail.into()) }
    }

    fn unknown(detail: impl Into<String>) -> Self {
        Self { value: None, confidence: DetectionConfidence::Unknown, detail: Some(detail.into()) }
    }

    #[cfg(not(windows))]
    fn unavailable(detail: impl Into<String>) -> Self {
        Self { value: None, confidence: DetectionConfidence::Unavailable, detail: Some(detail.into()) }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatingSystemSnapshot {
    pub name: Detection<String>,
    pub version: Detection<String>,
    pub architecture: Detection<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorSnapshot {
    pub architecture: Detection<String>,
    /// Available logical processor count; not a physical-core count.
    pub available_logical_processors: Detection<usize>,
    pub model: Detection<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    pub total_physical_bytes: Detection<u64>,
    pub available_physical_bytes: Detection<u64>,
    /// Not inferred from system RAM; requires a reliable adapter/runtime query.
    pub shared_gpu_bytes: Detection<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSnapshot {
    /// Free space on the configured/default model-storage volume only.
    pub free_bytes: Detection<u64>,
    pub total_bytes: Detection<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapterSnapshot {
    pub vendor: Detection<String>,
    pub model: Detection<String>,
    pub dedicated_vram_bytes: Detection<u64>,
    pub shared_memory_bytes: Detection<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccelerationBackend {
    Cpu,
    Cuda,
    DirectMl,
    Vulkan,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendSnapshot {
    pub backend: AccelerationBackend,
    /// GPU presence alone does not prove that an AI backend is installed/usable.
    pub availability: DetectionConfidence,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSnapshot {
    pub operating_system: OperatingSystemSnapshot,
    pub processor: ProcessorSnapshot,
    pub memory: MemorySnapshot,
    /// Unknown or empty does not imply that the PC has no GPU.
    pub graphics_adapters: Detection<Vec<GpuAdapterSnapshot>>,
    pub model_storage: DiskSnapshot,
    pub acceleration_backends: Vec<BackendSnapshot>,
    pub refreshed_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveProfile {
    Light,
    Balanced,
    Quality,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceAvailability {
    pub total_memory_bytes: Detection<u64>,
    pub available_memory_bytes: Detection<u64>,
    pub dedicated_vram_bytes: Detection<u64>,
    pub model_storage_free_bytes: Detection<u64>,
    pub acceleration_confirmed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreliminaryFeasibilityStatus {
    ResourceConstrained,
    ResourceHeadroomObserved,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreliminaryWorkloadFeasibility {
    pub status: PreliminaryFeasibilityStatus,
    /// Always false until a specific model and runtime are evaluated later.
    pub generation_support_confirmed: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveCapabilities {
    pub contract_version: u16,
    pub hardware: HardwareSnapshot,
    pub resources: ResourceAvailability,
    pub selected_profile: AdaptiveProfile,
    pub profile_confidence: DetectionConfidence,
    pub profile_reasons: Vec<String>,
    pub image_workload: PreliminaryWorkloadFeasibility,
    pub video_workload: PreliminaryWorkloadFeasibility,
}

/// Collect one fresh snapshot on demand. Only the model-storage location is
/// inspected; detection is read-only and performs no runtime/inference work.
pub fn collect(model_storage_path: &Path) -> AdaptiveCapabilities {
    let memory = hardware::memory_bytes();
    let disk = hardware::disk_space(model_storage_path);
    let adapters = detect_graphics_adapters();
    let cpu_count = std::thread::available_parallelism().ok().map(|count| count.get());

    let total_memory = memory.0.map(|bytes| Detection::confirmed(bytes, "Reported by the operating-system physical-memory API."))
        .unwrap_or_else(|| Detection::unknown("The operating system did not provide a total-memory reading."));
    let available_memory = memory.1.map(|bytes| Detection::confirmed(bytes, "Currently available physical memory reported by the operating system."))
        .unwrap_or_else(|| Detection::unknown("Currently available physical memory could not be read."));
    let disk_free = disk.free_bytes.map(|bytes| Detection::confirmed(bytes, "Free space available to the current user on the model-storage volume."))
        .unwrap_or_else(|| Detection::unknown(disk.unavailable_reason.unwrap_or("Free space could not be read.")));
    let disk_total = disk.total_bytes.map(|bytes| Detection::confirmed(bytes, "Filesystem capacity reported for the model-storage volume."))
        .unwrap_or_else(|| Detection::unknown("Filesystem capacity could not be read."));
    let processor_count = cpu_count.map(|count| Detection::confirmed(count, "Logical processors available to the current process."))
        .unwrap_or_else(|| Detection::unknown("The process could not obtain its available logical-processor count."));
    let architecture = architecture_detection();

    let dedicated_vram = adapters.value.as_ref().and_then(|devices| devices.first())
        .map(|adapter| adapter.dedicated_vram_bytes.clone())
        .unwrap_or_else(|| Detection::unknown("Dedicated GPU memory was not reliably exposed by the available OS/runtime APIs."));
    let backends = backend_snapshots();
    let acceleration_confirmed = backends.iter().any(|backend| backend.availability == DetectionConfidence::Confirmed);
    let resources = ResourceAvailability {
        total_memory_bytes: total_memory.clone(),
        available_memory_bytes: available_memory.clone(),
        dedicated_vram_bytes: dedicated_vram,
        model_storage_free_bytes: disk_free.clone(),
        acceleration_confirmed,
    };

    let hardware_snapshot = HardwareSnapshot {
        operating_system: OperatingSystemSnapshot {
            name: Detection::confirmed(std::env::consts::OS.to_string(), "Operating-system target reported by the application runtime."),
            version: Detection::unknown("OS release/build discovery is not enabled in this phase."),
            architecture: architecture.clone(),
        },
        processor: ProcessorSnapshot {
            architecture,
            available_logical_processors: processor_count,
            model: Detection::unknown("CPU model identification is not implemented; no registry or user files were inspected."),
        },
        memory: MemorySnapshot {
            total_physical_bytes: total_memory,
            available_physical_bytes: available_memory,
            shared_gpu_bytes: Detection::unknown("Shared GPU memory is not reliably distinguishable from system RAM without an evaluated graphics runtime."),
        },
        graphics_adapters: adapters,
        model_storage: DiskSnapshot { free_bytes: disk_free, total_bytes: disk_total },
        acceleration_backends: backends,
        refreshed_at_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH).ok().and_then(|duration| u64::try_from(duration.as_millis()).ok()),
    };

    let (selected_profile, profile_confidence, profile_reasons) = select_profile(&resources);
    let image_workload = assess_workload(
        &resources,
        IMAGE_SCREENING_FLOORS,
        "Image",
    );
    let video_workload = assess_workload(
        &resources,
        VIDEO_SCREENING_FLOORS,
        "Video",
    );
    AdaptiveCapabilities {
        contract_version: 1,
        hardware: hardware_snapshot,
        resources,
        selected_profile,
        profile_confidence,
        profile_reasons,
        image_workload,
        video_workload,
    }
}

/// Configurable, conservative resource screens. Profile hints are not model
/// memory estimates or guarantees; future phases should add model-specific
/// compatibility and working-set assessment rather than weakening these rules.
fn select_profile(resources: &ResourceAvailability) -> (AdaptiveProfile, DetectionConfidence, Vec<String>) {
    let (total, available, vram, storage) = (
        resources.total_memory_bytes.value,
        resources.available_memory_bytes.value,
        resources.dedicated_vram_bytes.value,
        resources.model_storage_free_bytes.value,
    );
    if total.is_none() || available.is_none() || vram.is_none() || storage.is_none() || !resources.acceleration_confirmed {
        let mut reasons = vec!["One or more readings or evaluated acceleration backends are unknown; selecting the conservative Light hint.".to_string()];
        if vram.is_none() { reasons.push("Dedicated GPU memory was not confirmed.".to_string()); }
        if !resources.acceleration_confirmed { reasons.push("No AI acceleration backend has been confirmed by an integrated runtime.".to_string()); }
        return (AdaptiveProfile::Light, DetectionConfidence::Unknown, reasons);
    }
    let (Some(total), Some(available), Some(vram), Some(storage)) = (total, available, vram, storage) else {
        return (AdaptiveProfile::Light, DetectionConfidence::Unknown, vec!["Incomplete resource evidence; using Light.".to_string()]);
    };
    let mut reasons = Vec::new();
    if total >= QUALITY_THRESHOLDS.total_ram
        && available >= QUALITY_THRESHOLDS.available_ram
        && vram >= QUALITY_THRESHOLDS.dedicated_vram
        && storage >= QUALITY_THRESHOLDS.storage_free
    {
        reasons.push("Reported RAM, available memory, VRAM, storage, and an evaluated acceleration backend meet conservative Quality hint thresholds.".to_string());
        (AdaptiveProfile::Quality, DetectionConfidence::Estimated, reasons)
    } else if total >= BALANCED_THRESHOLDS.total_ram
        && available >= BALANCED_THRESHOLDS.available_ram
        && vram >= BALANCED_THRESHOLDS.dedicated_vram
        && storage >= BALANCED_THRESHOLDS.storage_free
    {
        reasons.push("Reported resources meet conservative Balanced hint thresholds; model/runtime fit is not implied.".to_string());
        (AdaptiveProfile::Balanced, DetectionConfidence::Estimated, reasons)
    } else {
        reasons.push("Confirmed resource readings do not meet the conservative Balanced thresholds.".to_string());
        (AdaptiveProfile::Light, DetectionConfidence::Estimated, reasons)
    }
}

fn assess_workload(
    resources: &ResourceAvailability,
    floors: [u64; 3],
    label: &str,
) -> PreliminaryWorkloadFeasibility {
    let [total_floor, available_floor, storage_floor] = floors;
    let below_floor = resources.total_memory_bytes.value.is_some_and(|value| value < total_floor)
        || resources.available_memory_bytes.value.is_some_and(|value| value < available_floor)
        || resources.model_storage_free_bytes.value.is_some_and(|value| value < storage_floor);
    if below_floor {
        return PreliminaryWorkloadFeasibility {
            status: PreliminaryFeasibilityStatus::ResourceConstrained,
            generation_support_confirmed: false,
            reasons: vec![format!("Known memory or model-storage resources are below the preliminary {label} screening floor."), "This is only a resource warning and does not evaluate a specific model or runtime.".to_string()],
        };
    }
    let readings_known = resources.total_memory_bytes.value.is_some()
        && resources.available_memory_bytes.value.is_some()
        && resources.model_storage_free_bytes.value.is_some();
    if readings_known && resources.dedicated_vram_bytes.value.is_some() && resources.acceleration_confirmed {
        return PreliminaryWorkloadFeasibility {
            status: PreliminaryFeasibilityStatus::ResourceHeadroomObserved,
            generation_support_confirmed: false,
            reasons: vec!["Some preliminary system resources are present, but no model/runtime compatibility has been evaluated.".to_string()],
        };
    }
    PreliminaryWorkloadFeasibility {
        status: PreliminaryFeasibilityStatus::Unknown,
        generation_support_confirmed: false,
        reasons: vec!["Resource evidence is incomplete, or GPU/acceleration support has not been confirmed.".to_string(), "Specific model and runtime compatibility is deferred to a later phase.".to_string()],
    }
}

fn backend_snapshots() -> Vec<BackendSnapshot> {
    let detail = "Phase 1 does not install, probe, or assume an AI runtime; backend availability remains unknown.".to_string();
    vec![
        BackendSnapshot { backend: AccelerationBackend::Cpu, availability: DetectionConfidence::Unknown, detail: detail.clone() },
        BackendSnapshot { backend: AccelerationBackend::Cuda, availability: DetectionConfidence::Unknown, detail: detail.clone() },
        BackendSnapshot { backend: AccelerationBackend::DirectMl, availability: DetectionConfidence::Unknown, detail: detail.clone() },
        BackendSnapshot { backend: AccelerationBackend::Vulkan, availability: DetectionConfidence::Unknown, detail },
    ]
}

#[cfg(windows)]
fn architecture_detection() -> Detection<String> {
    use std::ffi::c_void;
    #[repr(C)]
    struct NativeSystemInfo {
        processor_architecture: u16,
        reserved: u16,
        page_size: u32,
        minimum_application_address: *mut c_void,
        maximum_application_address: *mut c_void,
        active_processor_mask: usize,
        number_of_processors: u32,
        processor_type: u32,
        allocation_granularity: u32,
        processor_level: u16,
        processor_revision: u16,
    }
    #[link(name = "kernel32")]
    extern "system" { fn GetNativeSystemInfo(info: *mut NativeSystemInfo); }

    let mut info: NativeSystemInfo = unsafe { std::mem::zeroed() };
    info.page_size = 0;
    // SAFETY: info is a writable structure matching SYSTEM_INFO and lives for
    // the duration of the documented synchronous Win32 call.
    unsafe { GetNativeSystemInfo(&mut info); }
    let name = match info.processor_architecture {
        0 => "x86",
        5 => "ARM",
        6 => "Itanium",
        9 => "x86_64",
        12 => "ARM64",
        _ => return Detection::unknown("Windows returned an unrecognized native processor architecture code."),
    };
    Detection::confirmed(name.to_string(), "Native Windows architecture reported by GetNativeSystemInfo.")
}

#[cfg(not(windows))]
fn architecture_detection() -> Detection<String> {
    Detection::confirmed(std::env::consts::ARCH.to_string(), "Application target architecture; native hardware architecture may differ under emulation.")
}

#[cfg(windows)]
fn detect_graphics_adapters() -> Detection<Vec<GpuAdapterSnapshot>> {
    use std::mem::size_of;
    use std::ptr;

    #[repr(C)]
    struct DisplayDeviceW {
        cb: u32,
        device_name: [u16; 32],
        device_string: [u16; 128],
        state_flags: u32,
        device_id: [u16; 128],
        device_key: [u16; 128],
    }
    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayDevicesW(device: *const u16, device_number: u32, display_device: *mut DisplayDeviceW, flags: u32) -> i32;
    }

    const MIRRORING_DRIVER: u32 = 0x0000_0008;
    let mut adapters = Vec::new();
    let mut enumeration_succeeded = false;
    for index in 0..32 {
        let mut device: DisplayDeviceW = unsafe { std::mem::zeroed() };
        device.cb = size_of::<DisplayDeviceW>() as u32;
        // SAFETY: device is correctly sized and writable; null requests the
        // system's adapter list according to the Win32 API contract.
        let found = unsafe { EnumDisplayDevicesW(ptr::null(), index, &mut device, 0) };
        if found == 0 { break; }
        enumeration_succeeded = true;
        if device.state_flags & MIRRORING_DRIVER != 0 { continue; }
        let model = nul_terminated_wide(&device.device_string);
        if model.trim().is_empty() { continue; }
        let device_id = nul_terminated_wide(&device.device_id);
        let vendor = vendor_from_device_id(&device_id);
        adapters.push(GpuAdapterSnapshot {
            vendor: vendor.map(|name| Detection::confirmed(name, "Mapped from the Windows Plug and Play PCI vendor identifier."))
                .unwrap_or_else(|| Detection::unknown("A recognized PCI vendor identifier was not present in the adapter device ID.")),
            model: Detection::confirmed(model, "Adapter description reported by Windows display-device enumeration."),
            dedicated_vram_bytes: Detection::unknown("The display enumeration API does not provide reliable dedicated VRAM capacity."),
            shared_memory_bytes: Detection::unknown("Shared GPU memory was not exposed by the display enumeration API."),
        });
    }
    if !adapters.is_empty() {
        Detection::confirmed(adapters, "One or more display adapters were enumerated by Windows. This does not confirm AI-runtime compatibility.")
    } else if enumeration_succeeded {
        Detection::unknown("Windows enumerated display devices but did not expose a usable physical adapter description; this does not prove no GPU exists.")
    } else {
        Detection::unknown("Windows display-device enumeration returned no adapter information; GPU absence is not inferred.")
    }
}

#[cfg(not(windows))]
fn detect_graphics_adapters() -> Detection<Vec<GpuAdapterSnapshot>> {
    Detection::unavailable("GPU enumeration is implemented only for the Windows desktop target in this phase.")
}

#[cfg(windows)]
fn nul_terminated_wide(value: &[u16]) -> String {
    let end = value.iter().position(|unit| *unit == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

#[cfg(windows)]
fn vendor_from_device_id(device_id: &str) -> Option<String> {
    let upper = device_id.to_ascii_uppercase();
    let start = upper.find("VEN_")? + 4;
    let vendor_id = upper.get(start..start + 4)?;
    let vendor = match vendor_id {
        "10DE" => "NVIDIA",
        "1002" | "1022" => "AMD",
        "8086" => "Intel",
        "1414" => "Microsoft",
        "13B5" => "ARM",
        "5143" => "Qualcomm",
        _ => return None,
    };
    Some(vendor.to_string())
}
