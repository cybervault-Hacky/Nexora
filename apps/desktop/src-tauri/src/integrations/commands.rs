use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    StandardDirectory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionConfidence {
    CandidateOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedInstallation {
    pub executable_path: String,
    /// `None` until a separate, explicitly approved version probe is implemented.
    pub version: Option<String>,
    pub method: DetectionMethod,
    pub confidence: DetectionConfidence,
    /// Explains the exact validation level; it is not a publisher signature claim.
    pub validation: String,
}

/// Tagged, renderer-safe result shared by first-party app detectors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InstallationDetectionResult {
    Found {
        installations: Vec<DetectedInstallation>,
        #[serde(rename = "incompleteLocations")]
        incomplete_locations: Vec<String>,
        #[serde(rename = "rejectedCandidates")]
        rejected_candidates: u32,
    },
    NotFound {
        #[serde(rename = "checkedLocations")]
        checked_locations: Vec<String>,
        #[serde(rename = "rejectedCandidates")]
        rejected_candidates: u32,
    },
    Unsupported {
        reason: String,
    },
    AccessDenied {
        reason: String,
        #[serde(rename = "incompleteLocations")]
        incomplete_locations: Vec<String>,
    },
    Error {
        code: String,
        message: String,
        #[serde(rename = "incompleteLocations")]
        incomplete_locations: Vec<String>,
    },
}

/// Runs only a bounded Windows directory detector. It never launches Blender,
/// consults the registry, or performs a recursive disk search.
#[tauri::command]
pub fn get_blender_detection_status() -> InstallationDetectionResult {
    super::detector::detect_blender_installations()
}
