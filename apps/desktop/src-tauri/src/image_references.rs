//! Persisted, transport-independent metadata for user-selected image references.
//! This describes preparation state only; it carries no image bytes or code.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageReferenceMetadata {
    pub schema_version: u16,
    pub reference_id: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub processing_state: String,
    #[serde(default)]
    pub validation_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedImageTo3dTask {
    pub schema_version: u16,
    pub task_type: String,
    pub user_request: String,
    pub references: Vec<ImageReferenceMetadata>,
    pub known_constraints: Vec<String>,
    pub missing_information: Vec<String>,
    pub analysis_available: bool,
    pub next_stage: String,
}

pub fn valid_reference(reference: &ImageReferenceMetadata) -> bool {
    reference.schema_version == 1
        && reference.reference_id.len() == 32
        && reference.reference_id.bytes().all(|b| b.is_ascii_hexdigit())
        && reference.format == "image/png"
        && reference.width > 0
        && reference.height > 0
        && reference.width <= 2_048
        && reference.height <= 2_048
        && u64::from(reference.width) * u64::from(reference.height) <= 4 * 1024 * 1024
        && reference.size_bytes > 0
        && reference.size_bytes <= 8 * 1024 * 1024
        && reference.description.as_ref().map_or(true, |value| value.chars().count() <= 32_000)
        && reference.processing_state == "validated"
        && reference.validation_errors.is_empty()
}

pub fn valid_task(task: &PreparedImageTo3dTask) -> bool {
    task.schema_version == 1
        && task.task_type == "image_to_3d"
        && !task.user_request.trim().is_empty()
        && task.user_request.chars().count() <= 32_000
        && task.references.len() == 1
        && task.references.iter().all(valid_reference)
        && task.known_constraints.len() <= 16
        && task.missing_information.len() <= 16
        && task.known_constraints.iter().chain(&task.missing_information)
            .all(|item| item.chars().count() <= 1_000)
        && !task.analysis_available
        && task.next_stage == "awaiting_supported_vision_capability"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> ImageReferenceMetadata {
        ImageReferenceMetadata {
            schema_version: 1,
            reference_id: "0123456789abcdef0123456789abcdef".into(),
            format: "image/png".into(),
            width: 512,
            height: 512,
            size_bytes: 128_000,
            description: Some("a stylized stone arch".into()),
            processing_state: "validated".into(),
            validation_errors: vec![],
        }
    }

    #[test]
    fn validates_versioned_reference_and_data_only_task() {
        let reference = reference();
        assert!(valid_reference(&reference));
        let task = PreparedImageTo3dTask {
            schema_version: 1,
            task_type: "image_to_3d".into(),
            user_request: "Create a stone arch inspired by the reference".into(),
            references: vec![reference],
            known_constraints: vec!["Image is locally validated".into()],
            missing_information: vec!["Exact dimensions are not specified".into()],
            analysis_available: false,
            next_stage: "awaiting_supported_vision_capability".into(),
        };
        assert!(valid_task(&task));
    }

    #[test]
    fn rejects_unverified_analysis_and_unsafe_reference_metadata() {
        let mut reference = reference();
        reference.reference_id = "../../outside".into();
        assert!(!valid_reference(&reference));
        reference = reference();
        reference.validation_errors.push("corrupt".into());
        assert!(!valid_reference(&reference));
        reference = reference();
        let mut task = PreparedImageTo3dTask {
            schema_version: 1,
            task_type: "image_to_3d".into(),
            user_request: "Make a 3D arch".into(),
            references: vec![reference],
            known_constraints: vec![],
            missing_information: vec![],
            analysis_available: true,
            next_stage: "awaiting_supported_vision_capability".into(),
        };
        assert!(!valid_task(&task));
        task.analysis_available = false;
        task.next_stage = "execute_python".into();
        assert!(!valid_task(&task));
    }
}
