//! Transport-independent Blender action contract and validation boundary.
//!
//! This module does not connect to Blender and does not execute Blender work.
//! Every accepted value is only a validated declarative action. The default
//! executor is deliberately unavailable until a separately approved connector
//! exists.

use std::collections::BTreeSet;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};

pub const ACTION_CONTRACT_VERSION: u64 = 1;
pub const MAX_CANDIDATE_BYTES: usize = 32 * 1024;
pub const MAX_ACTIONS_PER_BATCH: usize = 16;
pub const MAX_OBJECT_NAME_CHARS: usize = 64;
pub const MAX_OBJECT_ID_CHARS: usize = 64;
pub const MAX_REQUEST_ID_CHARS: usize = 64;
pub const MAX_ACTION_ID_CHARS: usize = 64;
pub const MAX_SCENE_LOCATION: f64 = 10_000.0;
pub const MAX_ROTATION_RADIANS: f64 = std::f64::consts::TAU * 4.0;
pub const MIN_OBJECT_SCALE: f64 = 0.001;
pub const MAX_OBJECT_SCALE: f64 = 1_000.0;
pub const DEFAULT_OBJECT_LIST_LIMIT: u16 = 100;
pub const MAX_OBJECT_LIST_LIMIT: u16 = 1_000;
pub const MIN_ANIMATION_FRAME: u32 = 1;
pub const MAX_ANIMATION_FRAME: u32 = 100_000;
pub const MAX_ANIMATION_FRAME_COUNT: u32 = 2_500;
pub const SUPPORTED_ANIMATION_FRAME_RATES: [u16; 6] = [24, 25, 30, 48, 50, 60];
pub const MAX_ANIMATION_DESCRIPTION_CHARS: usize = 1_024;
pub const MAX_ANIMATION_KEYFRAMES: usize = 8;

pub const GET_SCENE_SUMMARY_ID: &str = "blender.get_scene_summary";
pub const LIST_OBJECTS_ID: &str = "blender.list_objects";
pub const CREATE_PRIMITIVE_ID: &str = "blender.create_primitive";
pub const SET_OBJECT_TRANSFORM_ID: &str = "blender.set_object_transform";
pub const RENAME_OBJECT_ID: &str = "blender.rename_object";
pub const ANIMATE_OBJECT_ID: &str = "blender.animate_object";
pub const ANIMATE_OBJECT_KEYFRAMES_ID: &str = "blender.animate_object_keyframes";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationOperation {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpolationMode {
    Linear,
    Bezier,
    Constant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveType {
    Cube,
    UvSphere,
    Cylinder,
    Cone,
    Torus,
    Plane,
}

impl PrimitiveType {
    fn from_candidate(value: &str) -> Option<Self> {
        match value {
            "cube" => Some(Self::Cube),
            "uv_sphere" => Some(Self::UvSphere),
            "cylinder" => Some(Self::Cylinder),
            "cone" => Some(Self::Cone),
            "torus" => Some(Self::Torus),
            "plane" => Some(Self::Plane),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectKeyframe {
    pub frame: u32,
    pub value: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectTransform {
    pub location: Vector3,
    /// Euler angles, in radians.
    pub rotation: Vector3,
    /// Positive per-axis scale; zero and negative scale are not accepted.
    pub scale: Vector3,
}

/// A fully parsed action. Its variants are declarative; they do not call `bpy`.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidatedAction {
    GetSceneSummary,
    ListObjects {
        collection_name: Option<String>,
        limit: u16,
    },
    CreatePrimitive {
        primitive: PrimitiveType,
        name: Option<String>,
        transform: ObjectTransform,
    },
    SetObjectTransform {
        object_id: String,
        transform: ObjectTransform,
    },
    RenameObject {
        object_id: String,
        new_name: String,
    },
    AnimateObject {
        object_id: String,
        operation: AnimationOperation,
        start_frame: u32,
        end_frame: u32,
        frame_rate: u16,
        interpolation: InterpolationMode,
        start_value: Vector3,
        end_value: Vector3,
    },
    AnimateObjectKeyframes {
        object_id: String,
        operation: AnimationOperation,
        frame_rate: u16,
        interpolation: InterpolationMode,
        keyframes: Vec<ObjectKeyframe>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    ReadOnly,
    Mutation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPermission {
    SceneInspect,
    SceneModify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationRequirement {
    None,
    ExplicitUserConfirmation,
    DestructiveConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultSchema {
    SceneSummary,
    ObjectList,
    CreatedObject,
    UpdatedObject,
    RenamedObject,
    Animation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ActionDescriptor {
    pub action_id: &'static str,
    pub contract_version: u64,
    pub class: ActionClass,
    pub permission: ActionPermission,
    pub confirmation: ConfirmationRequirement,
    pub result_schema: ResultSchema,
}

impl ValidatedAction {
    pub fn action_id(&self) -> &'static str {
        self.descriptor().action_id
    }

    pub fn descriptor(&self) -> ActionDescriptor {
        match self {
            Self::GetSceneSummary => ActionDescriptor {
                action_id: GET_SCENE_SUMMARY_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::ReadOnly,
                permission: ActionPermission::SceneInspect,
                confirmation: ConfirmationRequirement::None,
                result_schema: ResultSchema::SceneSummary,
            },
            Self::ListObjects { .. } => ActionDescriptor {
                action_id: LIST_OBJECTS_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::ReadOnly,
                permission: ActionPermission::SceneInspect,
                confirmation: ConfirmationRequirement::None,
                result_schema: ResultSchema::ObjectList,
            },
            Self::CreatePrimitive { .. } => ActionDescriptor {
                action_id: CREATE_PRIMITIVE_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::Mutation,
                permission: ActionPermission::SceneModify,
                confirmation: ConfirmationRequirement::ExplicitUserConfirmation,
                result_schema: ResultSchema::CreatedObject,
            },
            Self::SetObjectTransform { .. } => ActionDescriptor {
                action_id: SET_OBJECT_TRANSFORM_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::Mutation,
                permission: ActionPermission::SceneModify,
                confirmation: ConfirmationRequirement::ExplicitUserConfirmation,
                result_schema: ResultSchema::UpdatedObject,
            },
            Self::RenameObject { .. } => ActionDescriptor {
                action_id: RENAME_OBJECT_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::Mutation,
                permission: ActionPermission::SceneModify,
                confirmation: ConfirmationRequirement::ExplicitUserConfirmation,
                result_schema: ResultSchema::RenamedObject,
            },
            Self::AnimateObject { .. } => ActionDescriptor {
                action_id: ANIMATE_OBJECT_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::Mutation,
                permission: ActionPermission::SceneModify,
                confirmation: ConfirmationRequirement::ExplicitUserConfirmation,
                result_schema: ResultSchema::Animation,
            },
            Self::AnimateObjectKeyframes { .. } => ActionDescriptor {
                action_id: ANIMATE_OBJECT_KEYFRAMES_ID,
                contract_version: ACTION_CONTRACT_VERSION,
                class: ActionClass::Mutation,
                permission: ActionPermission::SceneModify,
                confirmation: ConfirmationRequirement::ExplicitUserConfirmation,
                result_schema: ResultSchema::Animation,
            },
        }
    }
}

/// Typed future result shapes. No current code constructs or returns these as
/// evidence that Blender work happened.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneSummaryResult {
    pub scene_name: String,
    pub object_count: u32,
    pub collection_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlenderObjectKind {
    Mesh,
    Curve,
    Camera,
    Light,
    Empty,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ObjectSummary {
    pub object_id: String,
    pub name: String,
    pub object_kind: BlenderObjectKind,
    pub transform: ObjectTransform,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ObjectListResult {
    /// Bounded by `MAX_OBJECT_LIST_LIMIT`.
    pub objects: Vec<ObjectSummary>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MutationResult {
    pub object_id: String,
    pub name: String,
    pub transform: ObjectTransform,
}

/// Future executor result shape only; no Phase 9 path constructs this value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationResult {
    pub object_id: String,
    pub operation: AnimationOperation,
    pub start_frame: u32,
    pub end_frame: u32,
    pub frame_rate: u16,
    pub keyframe_count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "result_type", content = "result", rename_all = "snake_case")]
pub enum ActionResult {
    SceneSummary(SceneSummaryResult),
    ObjectList(ObjectListResult),
    CreatedObject(MutationResult),
    UpdatedObject(MutationResult),
    RenamedObject(MutationResult),
    Animation(AnimationResult),
}

/// Result codes are stable and intentionally do not include untrusted values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorCode {
    MalformedJson,
    PayloadTooLarge,
    InvalidType,
    MissingField,
    UnknownField,
    UnsupportedContractVersion,
    UnknownAction,
    EmptyBatch,
    TooManyActions,
    InvalidRequestId,
    InvalidIdentifier,
    InvalidString,
    StringTooLong,
    InvalidEnum,
    InvalidVectorLength,
    InvalidNumber,
    NonFiniteNumber,
    OutOfRange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub code: ValidationErrorCode,
    /// Stable JSON path; untrusted keys/values are never copied into this field.
    pub path: String,
}

impl ValidationError {
    fn new(code: ValidationErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

/// Constructible only by the validator: callers cannot execute raw JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedBatch {
    contract_version: u64,
    request_id: String,
    actions: Vec<ValidatedAction>,
}

impl ValidatedBatch {
    pub fn contract_version(&self) -> u64 {
        self.contract_version
    }

    /// Caller-supplied idempotency key. A future executor must bind it to the
    /// validated payload and reject conflicting/repeated mutations safely.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn actions(&self) -> &[ValidatedAction] {
        &self.actions
    }
}

/// Parse untrusted JSON while rejecting duplicate keys and enforcing a byte
/// bound before building the candidate tree.
pub fn parse_candidate_json(input: &str) -> Result<ValidatedBatch, Vec<ValidationError>> {
    if input.len() > MAX_CANDIDATE_BYTES {
        return Err(vec![ValidationError::new(
            ValidationErrorCode::PayloadTooLarge,
            "$",
        )]);
    }

    let strict_value = serde_json::from_str::<StrictJsonValue>(input).map_err(|_| {
        vec![ValidationError::new(
            ValidationErrorCode::MalformedJson,
            "$",
        )]
    })?;
    validate_candidate_value(&strict_value.into_value())
}

/// Validate an already-parsed candidate. External byte inputs should use
/// `parse_candidate_json` so duplicate keys and raw-size limits are enforced.
pub fn validate_candidate_value(candidate: &Value) -> Result<ValidatedBatch, Vec<ValidationError>> {
    let encoded_len = match serde_json::to_vec(candidate) {
        Ok(encoded) => encoded.len(),
        Err(_) => {
            return Err(vec![ValidationError::new(
                ValidationErrorCode::MalformedJson,
                "$",
            )]);
        }
    };
    if encoded_len > MAX_CANDIDATE_BYTES {
        return Err(vec![ValidationError::new(
            ValidationErrorCode::PayloadTooLarge,
            "$",
        )]);
    }

    let Some(root) = candidate.as_object() else {
        return Err(vec![ValidationError::new(
            ValidationErrorCode::InvalidType,
            "$",
        )]);
    };

    let mut errors = Vec::new();
    check_fields(
        root,
        "$",
        &["contract_version", "request_id", "actions"],
        &[],
        &mut errors,
    );
    let contract_version = parse_contract_version(
        root.get("contract_version"),
        "$.contract_version",
        &mut errors,
    );
    let request_id = parse_request_id(root.get("request_id"), "$.request_id", &mut errors);
    let actions_value = root.get("actions");

    if contract_version.is_some_and(|version| version != ACTION_CONTRACT_VERSION) {
        return Err(errors);
    }

    let Some(actions_value) = actions_value else {
        return Err(errors);
    };
    let Some(candidate_actions) = actions_value.as_array() else {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidType,
            "$.actions",
        ));
        return Err(errors);
    };
    if candidate_actions.is_empty() {
        errors.push(ValidationError::new(
            ValidationErrorCode::EmptyBatch,
            "$.actions",
        ));
        return Err(errors);
    }
    if candidate_actions.len() > MAX_ACTIONS_PER_BATCH {
        errors.push(ValidationError::new(
            ValidationErrorCode::TooManyActions,
            "$.actions",
        ));
        return Err(errors);
    }

    let mut validated_actions = Vec::with_capacity(candidate_actions.len());
    for (index, candidate_action) in candidate_actions.iter().enumerate() {
        match validate_action(candidate_action, index) {
            Ok(action) => validated_actions.push(action),
            Err(mut action_errors) => errors.append(&mut action_errors),
        }
    }

    if errors.is_empty() {
        if let Some(request_id) = request_id {
            return Ok(ValidatedBatch {
                contract_version: ACTION_CONTRACT_VERSION,
                request_id,
                actions: validated_actions,
            });
        }
    }

    Err(errors)
}

fn validate_action(
    candidate: &Value,
    index: usize,
) -> Result<ValidatedAction, Vec<ValidationError>> {
    let base_path = format!("$.actions[{index}]");
    let mut errors = Vec::new();
    let Some(action) = candidate.as_object() else {
        return Err(vec![ValidationError::new(
            ValidationErrorCode::InvalidType,
            base_path,
        )]);
    };

    check_fields(
        action,
        &base_path,
        &["action_id", "contract_version", "parameters"],
        &[],
        &mut errors,
    );
    let action_id = parse_bounded_string(
        action.get("action_id"),
        &format!("{base_path}.action_id"),
        1,
        MAX_ACTION_ID_CHARS,
        &mut errors,
    );
    let action_version = parse_contract_version(
        action.get("contract_version"),
        &format!("{base_path}.contract_version"),
        &mut errors,
    );
    let parameters_path = format!("{base_path}.parameters");
    let parameters = match action.get("parameters") {
        Some(value) => match value.as_object() {
            Some(object) => Some(object),
            None => {
                errors.push(ValidationError::new(
                    ValidationErrorCode::InvalidType,
                    parameters_path.clone(),
                ));
                None
            }
        },
        None => None,
    };

    if action_version.is_some_and(|version| version != ACTION_CONTRACT_VERSION) {
        return Err(errors);
    }

    let (Some(action_id), Some(parameters)) = (action_id, parameters) else {
        return Err(errors);
    };

    let validated = match action_id.as_str() {
        GET_SCENE_SUMMARY_ID => {
            check_fields(parameters, &parameters_path, &[], &[], &mut errors);
            Some(ValidatedAction::GetSceneSummary)
        }
        LIST_OBJECTS_ID => {
            check_fields(
                parameters,
                &parameters_path,
                &[],
                &["collection_name", "limit"],
                &mut errors,
            );
            let collection_name = parse_optional_bounded_string(
                parameters.get("collection_name"),
                &format!("{parameters_path}.collection_name"),
                1,
                MAX_OBJECT_NAME_CHARS,
                &mut errors,
            );
            let requested_limit = parse_optional_integer(
                parameters.get("limit"),
                &format!("{parameters_path}.limit"),
                &mut errors,
            );
            let limit = match requested_limit {
                Some(value) if (1..=u64::from(MAX_OBJECT_LIST_LIMIT)).contains(&value) => {
                    value as u16
                }
                Some(_) => {
                    errors.push(ValidationError::new(
                        ValidationErrorCode::OutOfRange,
                        format!("{parameters_path}.limit"),
                    ));
                    DEFAULT_OBJECT_LIST_LIMIT
                }
                None => DEFAULT_OBJECT_LIST_LIMIT,
            };
            Some(ValidatedAction::ListObjects {
                collection_name,
                limit,
            })
        }
        CREATE_PRIMITIVE_ID => {
            check_fields(
                parameters,
                &parameters_path,
                &["primitive", "location", "rotation", "scale"],
                &["name"],
                &mut errors,
            );
            let primitive = parse_primitive(
                parameters.get("primitive"),
                &format!("{parameters_path}.primitive"),
                &mut errors,
            );
            let name = parse_optional_bounded_string(
                parameters.get("name"),
                &format!("{parameters_path}.name"),
                1,
                MAX_OBJECT_NAME_CHARS,
                &mut errors,
            );
            let transform = parse_transform(parameters, &parameters_path, &mut errors);
            match (primitive, transform) {
                (Some(primitive), Some(transform)) => Some(ValidatedAction::CreatePrimitive {
                    primitive,
                    name,
                    transform,
                }),
                _ => None,
            }
        }
        SET_OBJECT_TRANSFORM_ID => {
            check_fields(
                parameters,
                &parameters_path,
                &["object_id", "location", "rotation", "scale"],
                &[],
                &mut errors,
            );
            let object_id = parse_object_id(
                parameters.get("object_id"),
                &format!("{parameters_path}.object_id"),
                &mut errors,
            );
            let transform = parse_transform(parameters, &parameters_path, &mut errors);
            match (object_id, transform) {
                (Some(object_id), Some(transform)) => Some(ValidatedAction::SetObjectTransform {
                    object_id,
                    transform,
                }),
                _ => None,
            }
        }
        RENAME_OBJECT_ID => {
            check_fields(
                parameters,
                &parameters_path,
                &["object_id", "new_name"],
                &[],
                &mut errors,
            );
            let object_id = parse_object_id(
                parameters.get("object_id"),
                &format!("{parameters_path}.object_id"),
                &mut errors,
            );
            let new_name = parse_bounded_string(
                parameters.get("new_name"),
                &format!("{parameters_path}.new_name"),
                1,
                MAX_OBJECT_NAME_CHARS,
                &mut errors,
            );
            match (object_id, new_name) {
                (Some(object_id), Some(new_name)) => Some(ValidatedAction::RenameObject {
                    object_id,
                    new_name,
                }),
                _ => None,
            }
        }
        ANIMATE_OBJECT_ID => {
            check_fields(
                parameters,
                &parameters_path,
                &[
                    "object_id",
                    "operation",
                    "start_frame",
                    "end_frame",
                    "frame_rate",
                    "interpolation",
                    "start_value",
                    "end_value",
                ],
                &[],
                &mut errors,
            );
            let object_id = parse_object_id(
                parameters.get("object_id"),
                &format!("{parameters_path}.object_id"),
                &mut errors,
            );
            let operation = parse_animation_operation(
                parameters.get("operation"),
                &format!("{parameters_path}.operation"),
                &mut errors,
            );
            let start_frame = parse_animation_frame(
                parameters.get("start_frame"),
                &format!("{parameters_path}.start_frame"),
                &mut errors,
            );
            let end_frame = parse_animation_frame(
                parameters.get("end_frame"),
                &format!("{parameters_path}.end_frame"),
                &mut errors,
            );
            let frame_rate = parse_animation_frame_rate(
                parameters.get("frame_rate"),
                &format!("{parameters_path}.frame_rate"),
                &mut errors,
            );
            let interpolation = parse_interpolation(
                parameters.get("interpolation"),
                &format!("{parameters_path}.interpolation"),
                &mut errors,
            );
            let value_bounds = operation.unwrap_or(AnimationOperation::Translation);
            let start_value = parse_animation_vector(
                parameters.get("start_value"),
                &format!("{parameters_path}.start_value"),
                value_bounds,
                &mut errors,
            );
            let end_value = parse_animation_vector(
                parameters.get("end_value"),
                &format!("{parameters_path}.end_value"),
                value_bounds,
                &mut errors,
            );
            if let (Some(start), Some(end)) = (start_frame, end_frame) {
                let inclusive_count = end.saturating_sub(start).saturating_add(1);
                if end <= start || inclusive_count > MAX_ANIMATION_FRAME_COUNT {
                    errors.push(ValidationError::new(
                        ValidationErrorCode::OutOfRange,
                        format!("{parameters_path}.end_frame"),
                    ));
                }
            }
            match (
                object_id,
                operation,
                start_frame,
                end_frame,
                frame_rate,
                interpolation,
                start_value,
                end_value,
            ) {
                (
                    Some(object_id),
                    Some(operation),
                    Some(start_frame),
                    Some(end_frame),
                    Some(frame_rate),
                    Some(interpolation),
                    Some(start_value),
                    Some(end_value),
                ) => Some(ValidatedAction::AnimateObject {
                    object_id,
                    operation,
                    start_frame,
                    end_frame,
                    frame_rate,
                    interpolation,
                    start_value,
                    end_value,
                }),
                _ => None,
            }
        }
        ANIMATE_OBJECT_KEYFRAMES_ID => {
            check_fields(
                parameters,
                &parameters_path,
                &[
                    "object_id",
                    "operation",
                    "frame_rate",
                    "interpolation",
                    "keyframes",
                ],
                &[],
                &mut errors,
            );
            let object_id = parse_object_id(
                parameters.get("object_id"),
                &format!("{parameters_path}.object_id"),
                &mut errors,
            );
            let operation = parse_animation_operation(
                parameters.get("operation"),
                &format!("{parameters_path}.operation"),
                &mut errors,
            );
            let frame_rate = parse_animation_frame_rate(
                parameters.get("frame_rate"),
                &format!("{parameters_path}.frame_rate"),
                &mut errors,
            );
            let interpolation = parse_interpolation(
                parameters.get("interpolation"),
                &format!("{parameters_path}.interpolation"),
                &mut errors,
            );
            let keyframes = parse_keyframe_points(
                parameters.get("keyframes"),
                &format!("{parameters_path}.keyframes"),
                operation.unwrap_or(AnimationOperation::Translation),
                &mut errors,
            );
            match (object_id, operation, frame_rate, interpolation, keyframes) {
                (
                    Some(object_id),
                    Some(operation),
                    Some(frame_rate),
                    Some(interpolation),
                    Some(keyframes),
                ) => Some(ValidatedAction::AnimateObjectKeyframes {
                    object_id,
                    operation,
                    frame_rate,
                    interpolation,
                    keyframes,
                }),
                _ => None,
            }
        }
        _ => {
            errors.push(ValidationError::new(
                ValidationErrorCode::UnknownAction,
                format!("{base_path}.action_id"),
            ));
            None
        }
    };

    if errors.is_empty() {
        if let Some(validated) = validated {
            return Ok(validated);
        }
    }
    Err(errors)
}

fn check_fields(
    object: &Map<String, Value>,
    path: &str,
    required: &[&str],
    optional: &[&str],
    errors: &mut Vec<ValidationError>,
) {
    for field in required {
        if !object.contains_key(*field) {
            errors.push(ValidationError::new(
                ValidationErrorCode::MissingField,
                format!("{path}.{field}"),
            ));
        }
    }
    for field in object.keys() {
        if !required.contains(&field.as_str()) && !optional.contains(&field.as_str()) {
            // Do not echo a model-provided key into an error path.
            errors.push(ValidationError::new(
                ValidationErrorCode::UnknownField,
                format!("{path}.<unknown>"),
            ));
        }
    }
}

fn parse_contract_version(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<u64> {
    let value = value?;
    let Some(version) = value.as_u64() else {
        errors.push(ValidationError::new(ValidationErrorCode::InvalidType, path));
        return None;
    };
    if version != ACTION_CONTRACT_VERSION {
        errors.push(ValidationError::new(
            ValidationErrorCode::UnsupportedContractVersion,
            path,
        ));
        return Some(version);
    }
    Some(version)
}

fn parse_request_id(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    let value = parse_bounded_string(value, path, 1, MAX_REQUEST_ID_CHARS, errors)?;
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._:-".contains(character))
    {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidRequestId,
            path,
        ));
        return None;
    }
    Some(value)
}

fn parse_object_id(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    let value = parse_bounded_string(value, path, 1, MAX_OBJECT_ID_CHARS, errors)?;
    let mut characters = value.chars();
    let first_is_valid = characters
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric());
    let rest_is_valid =
        characters.all(|character| character.is_ascii_alphanumeric() || "._:-".contains(character));
    if !first_is_valid || !rest_is_valid {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidIdentifier,
            path,
        ));
        return None;
    }
    Some(value)
}

fn parse_bounded_string(
    value: Option<&Value>,
    path: &str,
    minimum_chars: usize,
    maximum_chars: usize,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    let value = value?;
    let Some(text) = value.as_str() else {
        errors.push(ValidationError::new(ValidationErrorCode::InvalidType, path));
        return None;
    };
    let length = text.chars().count();
    if length < minimum_chars || text.trim().is_empty() || text.chars().any(char::is_control) {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidString,
            path,
        ));
        return None;
    }
    if length > maximum_chars {
        errors.push(ValidationError::new(
            ValidationErrorCode::StringTooLong,
            path,
        ));
        return None;
    }
    Some(text.to_owned())
}

fn parse_optional_bounded_string(
    value: Option<&Value>,
    path: &str,
    minimum_chars: usize,
    maximum_chars: usize,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    value.map(|value| {
        parse_bounded_string(Some(value), path, minimum_chars, maximum_chars, errors)
    })?
}

fn parse_optional_integer(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<u64> {
    let value = value?;
    match value.as_u64() {
        Some(integer) => Some(integer),
        None => {
            errors.push(ValidationError::new(ValidationErrorCode::InvalidType, path));
            None
        }
    }
}

fn parse_primitive(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<PrimitiveType> {
    let value = parse_bounded_string(value, path, 1, 16, errors)?;
    match PrimitiveType::from_candidate(&value) {
        Some(primitive) => Some(primitive),
        None => {
            errors.push(ValidationError::new(ValidationErrorCode::InvalidEnum, path));
            None
        }
    }
}

fn parse_animation_operation(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<AnimationOperation> {
    let value = parse_bounded_string(value, path, 1, 16, errors)?;
    match value.as_str() {
        "translation" => Some(AnimationOperation::Translation),
        "rotation" => Some(AnimationOperation::Rotation),
        "scale" => Some(AnimationOperation::Scale),
        _ => {
            errors.push(ValidationError::new(ValidationErrorCode::InvalidEnum, path));
            None
        }
    }
}

fn parse_interpolation(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<InterpolationMode> {
    let value = parse_bounded_string(value, path, 1, 16, errors)?;
    match value.as_str() {
        "linear" => Some(InterpolationMode::Linear),
        "bezier" => Some(InterpolationMode::Bezier),
        "constant" => Some(InterpolationMode::Constant),
        _ => {
            errors.push(ValidationError::new(ValidationErrorCode::InvalidEnum, path));
            None
        }
    }
}

fn parse_animation_frame(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<u32> {
    let value = parse_optional_integer(value, path, errors)?;
    if value < u64::from(MIN_ANIMATION_FRAME) || value > u64::from(MAX_ANIMATION_FRAME) {
        errors.push(ValidationError::new(ValidationErrorCode::OutOfRange, path));
        return None;
    }
    Some(value as u32)
}

fn parse_animation_frame_rate(
    value: Option<&Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<u16> {
    let value = parse_optional_integer(value, path, errors)?;
    let Ok(frame_rate) = u16::try_from(value) else {
        errors.push(ValidationError::new(ValidationErrorCode::OutOfRange, path));
        return None;
    };
    if !SUPPORTED_ANIMATION_FRAME_RATES.contains(&frame_rate) {
        errors.push(ValidationError::new(ValidationErrorCode::OutOfRange, path));
        return None;
    }
    Some(frame_rate)
}

fn parse_animation_vector(
    value: Option<&Value>,
    path: &str,
    operation: AnimationOperation,
    errors: &mut Vec<ValidationError>,
) -> Option<Vector3> {
    let (minimum, maximum) = match operation {
        AnimationOperation::Translation => (-MAX_SCENE_LOCATION, MAX_SCENE_LOCATION),
        AnimationOperation::Rotation => (-MAX_ROTATION_RADIANS, MAX_ROTATION_RADIANS),
        AnimationOperation::Scale => (MIN_OBJECT_SCALE, MAX_OBJECT_SCALE),
    };
    parse_vector(value, path, minimum, maximum, errors)
}

fn parse_keyframe_points(
    value: Option<&Value>,
    path: &str,
    operation: AnimationOperation,
    errors: &mut Vec<ValidationError>,
) -> Option<Vec<ObjectKeyframe>> {
    let Some(Value::Array(items)) = value else {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidType,
            path.to_string(),
        ));
        return None;
    };
    if !(2..=MAX_ANIMATION_KEYFRAMES).contains(&items.len()) {
        errors.push(ValidationError::new(
            ValidationErrorCode::OutOfRange,
            path.to_string(),
        ));
    }

    let mut keyframes = Vec::with_capacity(items.len().min(MAX_ANIMATION_KEYFRAMES));
    for (index, item) in items.iter().take(MAX_ANIMATION_KEYFRAMES).enumerate() {
        let keyframe_path = format!("{path}[{index}]");
        let Some(fields) = item.as_object() else {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidType,
                keyframe_path,
            ));
            continue;
        };
        check_fields(fields, &keyframe_path, &["frame", "value"], &[], errors);
        let frame = parse_animation_frame(
            fields.get("frame"),
            &format!("{keyframe_path}.frame"),
            errors,
        );
        let value = parse_animation_vector(
            fields.get("value"),
            &format!("{keyframe_path}.value"),
            operation,
            errors,
        );
        if let (Some(frame), Some(value)) = (frame, value) {
            keyframes.push(ObjectKeyframe { frame, value });
        }
    }

    if keyframes.len() >= 2 && keyframes.len() == items.len() {
        let mut valid_sequence = true;
        for pair in keyframes.windows(2) {
            if pair[1].frame <= pair[0].frame {
                valid_sequence = false;
                errors.push(ValidationError::new(
                    ValidationErrorCode::OutOfRange,
                    path.to_string(),
                ));
            }
        }
        let inclusive_count = keyframes
            .last()
            .zip(keyframes.first())
            .map(|(last, first)| last.frame.saturating_sub(first.frame).saturating_add(1));
        if inclusive_count.is_some_and(|count| count > MAX_ANIMATION_FRAME_COUNT) {
            valid_sequence = false;
            errors.push(ValidationError::new(
                ValidationErrorCode::OutOfRange,
                path.to_string(),
            ));
        }
        if valid_sequence {
            return Some(keyframes);
        }
    }
    None
}

fn parse_transform(
    parameters: &Map<String, Value>,
    base_path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<ObjectTransform> {
    let location = parse_vector(
        parameters.get("location"),
        &format!("{base_path}.location"),
        -MAX_SCENE_LOCATION,
        MAX_SCENE_LOCATION,
        errors,
    );
    let rotation = parse_vector(
        parameters.get("rotation"),
        &format!("{base_path}.rotation"),
        -MAX_ROTATION_RADIANS,
        MAX_ROTATION_RADIANS,
        errors,
    );
    let scale = parse_vector(
        parameters.get("scale"),
        &format!("{base_path}.scale"),
        MIN_OBJECT_SCALE,
        MAX_OBJECT_SCALE,
        errors,
    );
    match (location, rotation, scale) {
        (Some(location), Some(rotation), Some(scale)) => Some(ObjectTransform {
            location,
            rotation,
            scale,
        }),
        _ => None,
    }
}

fn parse_vector(
    value: Option<&Value>,
    path: &str,
    minimum: f64,
    maximum: f64,
    errors: &mut Vec<ValidationError>,
) -> Option<Vector3> {
    let value = value?;
    let Some(values) = value.as_array() else {
        errors.push(ValidationError::new(ValidationErrorCode::InvalidType, path));
        return None;
    };
    if values.len() != 3 {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidVectorLength,
            path,
        ));
        return None;
    }

    let x = parse_bounded_number(
        values.first(),
        &format!("{path}[0]"),
        minimum,
        maximum,
        errors,
    );
    let y = parse_bounded_number(
        values.get(1),
        &format!("{path}[1]"),
        minimum,
        maximum,
        errors,
    );
    let z = parse_bounded_number(
        values.get(2),
        &format!("{path}[2]"),
        minimum,
        maximum,
        errors,
    );
    match (x, y, z) {
        (Some(x), Some(y), Some(z)) => Some(Vector3 { x, y, z }),
        _ => None,
    }
}

fn parse_bounded_number(
    value: Option<&Value>,
    path: &str,
    minimum: f64,
    maximum: f64,
    errors: &mut Vec<ValidationError>,
) -> Option<f64> {
    let value = value?;
    let Some(number) = value.as_number().and_then(Number::as_f64) else {
        errors.push(ValidationError::new(
            ValidationErrorCode::InvalidNumber,
            path,
        ));
        return None;
    };
    if !number.is_finite() {
        errors.push(ValidationError::new(
            ValidationErrorCode::NonFiniteNumber,
            path,
        ));
        return None;
    }
    if number < minimum || number > maximum {
        errors.push(ValidationError::new(ValidationErrorCode::OutOfRange, path));
        return None;
    }
    Some(number)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectorApproval {
    #[default]
    Unavailable,
    Unapproved,
    Approved,
}

#[derive(Clone, Debug, Default)]
pub struct ExecutionContext {
    /// This is host-owned policy input, not renderer- or model-supplied state.
    pub connector: ConnectorApproval,
    pub granted_permissions: BTreeSet<ActionPermission>,
    /// The exact validated batch approved by a future host consent flow.
    /// Equality binds approval to its contents, not only a reusable request ID.
    /// No consent UI or durable permission store is implemented here.
    pub confirmed_batch: Option<ValidatedBatch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizationError {
    ConnectorUnavailable,
    ConnectorUnapproved,
    PermissionNotGranted(ActionPermission),
    ConfirmationRequired,
    DestructiveConfirmationRequired,
}

/// A batch crosses into an executor only after every action passes the same
/// host-owned connector, permission, and confirmation checks.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthorizedBatch {
    batch: ValidatedBatch,
}

impl AuthorizedBatch {
    pub fn batch(&self) -> &ValidatedBatch {
        &self.batch
    }
}

pub fn authorize_batch(
    batch: &ValidatedBatch,
    context: &ExecutionContext,
) -> Result<AuthorizedBatch, AuthorizationError> {
    match context.connector {
        ConnectorApproval::Unavailable => return Err(AuthorizationError::ConnectorUnavailable),
        ConnectorApproval::Unapproved => return Err(AuthorizationError::ConnectorUnapproved),
        ConnectorApproval::Approved => {}
    }

    let exact_batch_confirmed = context.confirmed_batch.as_ref() == Some(batch);
    for action in &batch.actions {
        let descriptor = action.descriptor();
        if !context.granted_permissions.contains(&descriptor.permission) {
            return Err(AuthorizationError::PermissionNotGranted(
                descriptor.permission,
            ));
        }
        match descriptor.confirmation {
            ConfirmationRequirement::None => {}
            ConfirmationRequirement::ExplicitUserConfirmation => {
                if !exact_batch_confirmed {
                    return Err(AuthorizationError::ConfirmationRequired);
                }
            }
            ConfirmationRequirement::DestructiveConfirmation => {
                if !exact_batch_confirmed {
                    return Err(AuthorizationError::DestructiveConfirmationRequired);
                }
            }
        }
    }

    Ok(AuthorizedBatch {
        batch: batch.clone(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionError {
    ConnectorUnavailable,
}

impl ExecutionError {
    pub fn code(self) -> &'static str {
        match self {
            Self::ConnectorUnavailable => "connector_unavailable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExecutionReceipt {
    pub request_id: String,
    pub results: Vec<ActionResult>,
}

/// Future executor boundary. Implementations must report a receipt only after
/// real connector evidence; the only implementation in this phase refuses.
pub trait BlenderActionExecutor: Send + Sync {
    fn execute(&self, batch: &AuthorizedBatch) -> Result<ExecutionReceipt, ExecutionError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableBlenderExecutor;

impl BlenderActionExecutor for UnavailableBlenderExecutor {
    fn execute(&self, _batch: &AuthorizedBatch) -> Result<ExecutionReceipt, ExecutionError> {
        Err(ExecutionError::ConnectorUnavailable)
    }
}

/// Strict JSON tree that detects duplicate keys before conversion to `Value`.
enum StrictJsonValue {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<StrictJsonValue>),
    Object(std::collections::BTreeMap<String, StrictJsonValue>),
}

impl<'de> Deserialize<'de> for StrictJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrictValueVisitor;

        impl<'de> Visitor<'de> for StrictValueVisitor {
            type Value = StrictJsonValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a strict JSON value without duplicate object keys")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::Number(Number::from(value)))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::Number(Number::from(value)))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Number::from_f64(value)
                    .map(StrictJsonValue::Number)
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue::String(value))
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(StrictJsonValue::Array(values))
            }

            fn visit_map<A>(self, mut mapping: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = std::collections::BTreeMap::new();
                while let Some((key, value)) = mapping.next_entry::<String, StrictJsonValue>()? {
                    if values.insert(key, value).is_some() {
                        return Err(<A::Error as de::Error>::custom("duplicate JSON object key"));
                    }
                }
                Ok(StrictJsonValue::Object(values))
            }
        }

        deserializer.deserialize_any(StrictValueVisitor)
    }
}

impl StrictJsonValue {
    fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(value),
            Self::Number(value) => Value::Number(value),
            Self::String(value) => Value::String(value),
            Self::Array(values) => Value::Array(values.into_iter().map(Self::into_value).collect()),
            Self::Object(values) => Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, value.into_value()))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn action(action_id: &str, parameters: Value) -> Value {
        json!({
            "action_id": action_id,
            "contract_version": ACTION_CONTRACT_VERSION,
            "parameters": parameters
        })
    }

    fn valid_transform() -> Value {
        json!({
            "location": [0.0, 1.0, -2.0],
            "rotation": [0.0, 0.5, -0.5],
            "scale": [1.0, 1.0, 1.0]
        })
    }

    fn valid_candidate(actions: Vec<Value>) -> Value {
        json!({
            "contract_version": ACTION_CONTRACT_VERSION,
            "request_id": "phase6-test-001",
            "actions": actions
        })
    }

    fn valid_batch(actions: Vec<Value>) -> ValidatedBatch {
        validate_candidate_value(&valid_candidate(actions)).expect("valid test batch")
    }

    fn contains_code(errors: &[ValidationError], expected: ValidationErrorCode) -> bool {
        errors.iter().any(|error| error.code == expected)
    }

    fn create_action() -> Value {
        let mut parameters = valid_transform();
        parameters["primitive"] = json!("cube");
        parameters["name"] = json!("Block");
        action(CREATE_PRIMITIVE_ID, parameters)
    }

    fn animation_action() -> Value {
        action(
            ANIMATE_OBJECT_ID,
            json!({
                "object_id": "obj:stable-01",
                "operation": "translation",
                "start_frame": 1,
                "end_frame": 48,
                "frame_rate": 24,
                "interpolation": "linear",
                "start_value": [0.0, 0.0, 0.0],
                "end_value": [10.0, 2.0, -1.0]
            }),
        )
    }

    fn keyframe_action() -> Value {
        action(
            ANIMATE_OBJECT_KEYFRAMES_ID,
            json!({
                "object_id": "obj:stable-01",
                "operation": "translation",
                "frame_rate": 24,
                "interpolation": "bezier",
                "keyframes": [
                    { "frame": 1, "value": [0.0, 0.0, 0.0] },
                    { "frame": 12, "value": [2.0, 0.0, 0.0] },
                    { "frame": 48, "value": [10.0, 2.0, -1.0] }
                ]
            }),
        )
    }

    #[test]
    fn accepts_all_six_typed_action_contracts() {
        let candidate = valid_candidate(vec![
            action(GET_SCENE_SUMMARY_ID, json!({})),
            action(
                LIST_OBJECTS_ID,
                json!({"collection_name": "Props", "limit": 20}),
            ),
            create_action(),
            action(SET_OBJECT_TRANSFORM_ID, {
                let mut parameters = valid_transform();
                parameters["object_id"] = json!("obj:stable-01");
                parameters
            }),
            action(
                RENAME_OBJECT_ID,
                json!({"object_id": "obj:stable-01", "new_name": "New name"}),
            ),
            animation_action(),
            keyframe_action(),
        ]);

        let batch = validate_candidate_value(&candidate).expect("all contracts should validate");
        assert_eq!(batch.contract_version(), ACTION_CONTRACT_VERSION);
        assert_eq!(batch.actions().len(), 7);
        assert_eq!(
            batch
                .actions()
                .iter()
                .map(ValidatedAction::action_id)
                .collect::<Vec<_>>(),
            vec![
                GET_SCENE_SUMMARY_ID,
                LIST_OBJECTS_ID,
                CREATE_PRIMITIVE_ID,
                SET_OBJECT_TRANSFORM_ID,
                RENAME_OBJECT_ID,
                ANIMATE_OBJECT_ID,
                ANIMATE_OBJECT_KEYFRAMES_ID,
            ]
        );
    }

    #[test]
    fn animation_action_is_bounded_typed_and_mutation_gated() {
        let batch = valid_batch(vec![animation_action()]);
        assert!(matches!(
            &batch.actions()[0],
            ValidatedAction::AnimateObject {
                operation: AnimationOperation::Translation,
                start_frame: 1,
                end_frame: 48,
                frame_rate: 24,
                interpolation: InterpolationMode::Linear,
                ..
            }
        ));
        let descriptor = batch.actions()[0].descriptor();
        assert_eq!(descriptor.action_id, ANIMATE_OBJECT_ID);
        assert_eq!(descriptor.class, ActionClass::Mutation);
        assert_eq!(descriptor.permission, ActionPermission::SceneModify);
        assert_eq!(
            descriptor.confirmation,
            ConfirmationRequirement::ExplicitUserConfirmation
        );
        let mut context = ExecutionContext {
            connector: ConnectorApproval::Approved,
            ..ExecutionContext::default()
        };
        context
            .granted_permissions
            .insert(ActionPermission::SceneModify);
        context.confirmed_batch = Some(batch.clone());
        let authorized = authorize_batch(&batch, &context).expect("exact batch confirmation");
        assert_eq!(
            UnavailableBlenderExecutor.execute(&authorized),
            Err(ExecutionError::ConnectorUnavailable)
        );

        let mut invalid_frame_order = animation_action();
        invalid_frame_order["parameters"]["end_frame"] = json!(1);
        assert!(contains_code(
            &validate_candidate_value(&valid_candidate(vec![invalid_frame_order]))
                .expect_err("reversed/equal range must be rejected"),
            ValidationErrorCode::OutOfRange
        ));

        let mut excessive_range = animation_action();
        excessive_range["parameters"]["end_frame"] =
            json!(MIN_ANIMATION_FRAME + MAX_ANIMATION_FRAME_COUNT);
        assert!(contains_code(
            &validate_candidate_value(&valid_candidate(vec![excessive_range]))
                .expect_err("oversized frame range must be rejected"),
            ValidationErrorCode::OutOfRange
        ));

        for (field, value) in [
            ("frame_rate", json!(23)),
            ("interpolation", json!("cubic")),
            ("operation", json!("location")),
        ] {
            let mut invalid = animation_action();
            invalid["parameters"][field] = value;
            let errors = validate_candidate_value(&valid_candidate(vec![invalid]))
                .expect_err("unsupported animation values must fail");
            assert!(contains_code(
                &errors,
                if field == "interpolation" || field == "operation" {
                    ValidationErrorCode::InvalidEnum
                } else {
                    ValidationErrorCode::OutOfRange
                }
            ));
        }

        let mut invalid_scale = animation_action();
        invalid_scale["parameters"]["operation"] = json!("scale");
        invalid_scale["parameters"]["start_value"] = json!([0.0, 1.0, 1.0]);
        assert!(contains_code(
            &validate_candidate_value(&valid_candidate(vec![invalid_scale]))
                .expect_err("non-positive animated scale must fail"),
            ValidationErrorCode::OutOfRange
        ));
    }

    #[test]
    fn keyframe_sequence_action_is_bounded_and_explicitly_confirmation_gated() {
        let batch = valid_batch(vec![keyframe_action()]);
        let ValidatedAction::AnimateObjectKeyframes { keyframes, .. } = &batch.actions()[0] else {
            panic!("expected typed keyframe sequence");
        };
        assert_eq!(keyframes.len(), 3);
        assert_eq!(keyframes[1].frame, 12);
        let descriptor = batch.actions()[0].descriptor();
        assert_eq!(descriptor.action_id, ANIMATE_OBJECT_KEYFRAMES_ID);
        assert_eq!(descriptor.permission, ActionPermission::SceneModify);
        assert_eq!(
            descriptor.confirmation,
            ConfirmationRequirement::ExplicitUserConfirmation
        );

        let mut context = ExecutionContext {
            connector: ConnectorApproval::Approved,
            ..ExecutionContext::default()
        };
        context
            .granted_permissions
            .insert(ActionPermission::SceneModify);
        assert_eq!(
            authorize_batch(&batch, &context),
            Err(AuthorizationError::ConfirmationRequired)
        );
        context.confirmed_batch = Some(batch.clone());
        let authorized = authorize_batch(&batch, &context).expect("exact sequence confirmation");
        assert_eq!(
            UnavailableBlenderExecutor.execute(&authorized),
            Err(ExecutionError::ConnectorUnavailable)
        );

        let mut changed_sequence = keyframe_action();
        changed_sequence["parameters"]["keyframes"][1]["value"] = json!([3.0, 0.0, 0.0]);
        let changed_batch = valid_batch(vec![changed_sequence]);
        assert_eq!(batch.request_id(), changed_batch.request_id());
        assert_eq!(
            authorize_batch(&changed_batch, &context),
            Err(AuthorizationError::ConfirmationRequired)
        );

        let mut reversed = keyframe_action();
        reversed["parameters"]["keyframes"][1]["frame"] = json!(1);
        assert!(contains_code(
            &validate_candidate_value(&valid_candidate(vec![reversed]))
                .expect_err("non-increasing keyframes must be rejected"),
            ValidationErrorCode::OutOfRange
        ));

        let mut too_many = keyframe_action();
        too_many["parameters"]["keyframes"] = json!((0..=MAX_ANIMATION_KEYFRAMES)
            .map(|index| { json!({ "frame": index + 1, "value": [index as f64, 0.0, 0.0] }) })
            .collect::<Vec<_>>());
        assert!(contains_code(
            &validate_candidate_value(&valid_candidate(vec![too_many]))
                .expect_err("too many keyframes must be rejected"),
            ValidationErrorCode::OutOfRange
        ));

        let mut excessive_span = keyframe_action();
        excessive_span["parameters"]["keyframes"][2]["frame"] =
            json!(MAX_ANIMATION_FRAME_COUNT + 1);
        assert!(contains_code(
            &validate_candidate_value(&valid_candidate(vec![excessive_span]))
                .expect_err("excessive keyframe span must be rejected"),
            ValidationErrorCode::OutOfRange
        ));
    }

    #[test]
    fn rejects_unknown_action_names() {
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            "blender.run_python",
            json!({"source": "pass"}),
        )]))
        .expect_err("arbitrary action names must be rejected");
        assert!(contains_code(&errors, ValidationErrorCode::UnknownAction));
    }

    #[test]
    fn rejects_unsupported_batch_and_action_versions() {
        let mut candidate = valid_candidate(vec![action(GET_SCENE_SUMMARY_ID, json!({}))]);
        candidate["contract_version"] = json!(ACTION_CONTRACT_VERSION + 1);
        let errors = validate_candidate_value(&candidate).expect_err("batch version must fail");
        assert!(contains_code(
            &errors,
            ValidationErrorCode::UnsupportedContractVersion
        ));

        let mut bad_action = action(GET_SCENE_SUMMARY_ID, json!({}));
        bad_action["contract_version"] = json!(ACTION_CONTRACT_VERSION + 1);
        let errors = validate_candidate_value(&valid_candidate(vec![bad_action]))
            .expect_err("action version must fail");
        assert!(contains_code(
            &errors,
            ValidationErrorCode::UnsupportedContractVersion
        ));
    }

    #[test]
    fn rejects_missing_and_extra_fields_at_each_contract_level() {
        let mut missing = create_action();
        missing["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("scale");
        let errors = validate_candidate_value(&valid_candidate(vec![missing]))
            .expect_err("required field must fail");
        assert!(contains_code(&errors, ValidationErrorCode::MissingField));

        let mut extra = create_action();
        extra["parameters"]["unexpected"] = json!(true);
        let errors = validate_candidate_value(&valid_candidate(vec![extra]))
            .expect_err("unknown parameter must fail");
        assert!(contains_code(&errors, ValidationErrorCode::UnknownField));

        let mut extra_root = valid_candidate(vec![action(GET_SCENE_SUMMARY_ID, json!({}))]);
        extra_root["arbitrary_code"] = json!("ignored fields are not allowed");
        let errors = validate_candidate_value(&extra_root).expect_err("unknown root must fail");
        assert!(contains_code(&errors, ValidationErrorCode::UnknownField));
    }

    #[test]
    fn rejects_incorrectly_typed_values() {
        let mut bad_transform = valid_transform();
        bad_transform["location"] = json!("0,0,0");
        bad_transform["primitive"] = json!("cube");
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            bad_transform,
        )]))
        .expect_err("vector string is not a typed vector");
        assert!(contains_code(&errors, ValidationErrorCode::InvalidType));

        let errors = validate_candidate_value(&valid_candidate(vec![action(
            LIST_OBJECTS_ID,
            json!({"limit": true}),
        )]))
        .expect_err("boolean is not an integer limit");
        assert!(contains_code(&errors, ValidationErrorCode::InvalidType));
    }

    #[test]
    fn rejects_out_of_range_locations_rotations_and_scales() {
        let mut parameters = valid_transform();
        parameters["primitive"] = json!("cube");
        parameters["location"] = json!([MAX_SCENE_LOCATION + 1.0, 0.0, 0.0]);
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            parameters,
        )]))
        .expect_err("location bound must be enforced");
        assert!(contains_code(&errors, ValidationErrorCode::OutOfRange));

        let mut parameters = valid_transform();
        parameters["primitive"] = json!("cube");
        parameters["rotation"] = json!([0.0, MAX_ROTATION_RADIANS + 1.0, 0.0]);
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            parameters,
        )]))
        .expect_err("rotation bound must be enforced");
        assert!(contains_code(&errors, ValidationErrorCode::OutOfRange));

        let mut parameters = valid_transform();
        parameters["primitive"] = json!("cube");
        parameters["scale"] = json!([1.0, 0.0, 1.0]);
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            parameters,
        )]))
        .expect_err("zero scale must be rejected");
        assert!(contains_code(&errors, ValidationErrorCode::OutOfRange));
    }

    #[test]
    fn rejects_wrong_vector_dimensions_and_primitive_values() {
        let mut parameters = valid_transform();
        parameters["primitive"] = json!("cube");
        parameters["location"] = json!([0.0, 1.0]);
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            parameters,
        )]))
        .expect_err("vectors must have exactly three values");
        assert!(contains_code(
            &errors,
            ValidationErrorCode::InvalidVectorLength
        ));

        let mut parameters = valid_transform();
        parameters["primitive"] = json!("arbitrary_operator");
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            parameters,
        )]))
        .expect_err("primitive must come from the enum");
        assert!(contains_code(&errors, ValidationErrorCode::InvalidEnum));
    }

    #[test]
    fn rejects_non_finite_json_numbers_and_duplicate_keys() {
        let template = r#"{"contract_version":1,"request_id":"r1","actions":[{"action_id":"blender.create_primitive","contract_version":1,"parameters":{"primitive":"cube","location":[NONFINITE,0,0],"rotation":[0,0,0],"scale":[1,1,1]}}]}"#;
        for value in ["NaN", "Infinity", "-Infinity", "1e999"] {
            let candidate = template.replace("NONFINITE", value);
            let errors = parse_candidate_json(&candidate)
                .expect_err("non-finite and overflowing numbers must be rejected");
            assert!(contains_code(&errors, ValidationErrorCode::MalformedJson));
        }

        let duplicate_key =
            r#"{"contract_version":1,"contract_version":1,"request_id":"r1","actions":[]}"#;
        let errors =
            parse_candidate_json(duplicate_key).expect_err("duplicate keys must be rejected");
        assert!(contains_code(&errors, ValidationErrorCode::MalformedJson));
    }

    #[test]
    fn rejects_oversized_strings_payloads_and_batches() {
        let mut parameters = valid_transform();
        parameters["primitive"] = json!("cube");
        parameters["name"] = json!("x".repeat(MAX_OBJECT_NAME_CHARS + 1));
        let errors = validate_candidate_value(&valid_candidate(vec![action(
            CREATE_PRIMITIVE_ID,
            parameters,
        )]))
        .expect_err("overlong object name must fail");
        assert!(contains_code(&errors, ValidationErrorCode::StringTooLong));

        let oversized = format!("{}{}", " ".repeat(MAX_CANDIDATE_BYTES + 1), "{}");
        let errors = parse_candidate_json(&oversized).expect_err("raw payload must be bounded");
        assert!(contains_code(&errors, ValidationErrorCode::PayloadTooLarge));

        let actions = (0..=MAX_ACTIONS_PER_BATCH)
            .map(|_| action(GET_SCENE_SUMMARY_ID, json!({})))
            .collect();
        let errors = validate_candidate_value(&valid_candidate(actions))
            .expect_err("action batch limit must be enforced");
        assert!(contains_code(&errors, ValidationErrorCode::TooManyActions));
    }

    #[test]
    fn rejects_empty_batches_and_invalid_request_ids() {
        let errors = validate_candidate_value(&valid_candidate(Vec::new()))
            .expect_err("empty batches are not executable");
        assert!(contains_code(&errors, ValidationErrorCode::EmptyBatch));

        let mut candidate = valid_candidate(vec![action(GET_SCENE_SUMMARY_ID, json!({}))]);
        candidate["request_id"] = json!("id with spaces");
        let errors =
            validate_candidate_value(&candidate).expect_err("request IDs are bounded tokens");
        assert!(contains_code(
            &errors,
            ValidationErrorCode::InvalidRequestId
        ));
    }

    #[test]
    fn list_objects_optional_filters_default_to_a_bounded_limit() {
        let batch = valid_batch(vec![action(LIST_OBJECTS_ID, json!({}))]);
        assert!(matches!(
            batch.actions().first(),
            Some(ValidatedAction::ListObjects {
                collection_name: None,
                limit: DEFAULT_OBJECT_LIST_LIMIT
            })
        ));
    }

    #[test]
    fn read_only_and_mutating_actions_have_distinct_policy_metadata() {
        let mut transform = valid_transform();
        transform["object_id"] = json!("obj:stable-01");
        let batch = valid_batch(vec![
            action(GET_SCENE_SUMMARY_ID, json!({})),
            action(LIST_OBJECTS_ID, json!({})),
            create_action(),
            action(SET_OBJECT_TRANSFORM_ID, transform),
            action(
                RENAME_OBJECT_ID,
                json!({"object_id": "obj:stable-01", "new_name": "Renamed"}),
            ),
            animation_action(),
        ]);
        for action in &batch.actions()[..2] {
            let policy = action.descriptor();
            assert_eq!(policy.class, ActionClass::ReadOnly);
            assert_eq!(policy.permission, ActionPermission::SceneInspect);
            assert_eq!(policy.confirmation, ConfirmationRequirement::None);
        }
        for action in &batch.actions()[2..] {
            let policy = action.descriptor();
            assert_eq!(policy.class, ActionClass::Mutation);
            assert_eq!(policy.permission, ActionPermission::SceneModify);
            assert_eq!(
                policy.confirmation,
                ConfirmationRequirement::ExplicitUserConfirmation
            );
        }
    }

    #[test]
    fn authorization_requires_connector_permissions_and_mutation_confirmation() {
        let read_batch = valid_batch(vec![action(GET_SCENE_SUMMARY_ID, json!({}))]);
        let mut context = ExecutionContext {
            connector: ConnectorApproval::Approved,
            ..ExecutionContext::default()
        };
        assert!(authorize_batch(&read_batch, &context).is_err());
        context
            .granted_permissions
            .insert(ActionPermission::SceneInspect);
        assert!(authorize_batch(&read_batch, &context).is_ok());

        let mutation_batch = valid_batch(vec![create_action()]);
        let mut mutation_context = ExecutionContext {
            connector: ConnectorApproval::Approved,
            ..ExecutionContext::default()
        };
        mutation_context
            .granted_permissions
            .insert(ActionPermission::SceneModify);
        assert_eq!(
            authorize_batch(&mutation_batch, &mutation_context),
            Err(AuthorizationError::ConfirmationRequired)
        );
        mutation_context.confirmed_batch = Some(mutation_batch.clone());
        assert!(authorize_batch(&mutation_batch, &mutation_context).is_ok());

        let mut different_actions = create_action();
        different_actions["parameters"]["name"] = json!("Different name");
        let same_request_id_different_payload = valid_batch(vec![different_actions]);
        assert_eq!(
            same_request_id_different_payload.request_id(),
            mutation_batch.request_id()
        );
        assert_eq!(
            authorize_batch(&same_request_id_different_payload, &mutation_context),
            Err(AuthorizationError::ConfirmationRequired)
        );
    }

    #[test]
    fn unavailable_or_unapproved_connector_denies_the_entire_batch() {
        let batch = valid_batch(vec![action(GET_SCENE_SUMMARY_ID, json!({}))]);
        assert_eq!(
            authorize_batch(&batch, &ExecutionContext::default()),
            Err(AuthorizationError::ConnectorUnavailable)
        );
        let context = ExecutionContext {
            connector: ConnectorApproval::Unapproved,
            ..ExecutionContext::default()
        };
        assert_eq!(
            authorize_batch(&batch, &context),
            Err(AuthorizationError::ConnectorUnapproved)
        );
    }

    #[test]
    fn batch_validation_is_all_or_nothing() {
        let candidate = valid_candidate(vec![
            action(GET_SCENE_SUMMARY_ID, json!({})),
            action("blender.unknown", json!({})),
        ]);
        let errors = validate_candidate_value(&candidate)
            .expect_err("one invalid action must reject the complete batch");
        assert!(contains_code(&errors, ValidationErrorCode::UnknownAction));
        // The API returns no ValidatedBatch at all; no prefix can reach policy/execution.
    }

    #[test]
    fn unavailable_executor_never_reports_a_success_receipt() {
        let batch = valid_batch(vec![action(GET_SCENE_SUMMARY_ID, json!({}))]);
        let mut context = ExecutionContext {
            connector: ConnectorApproval::Approved,
            ..ExecutionContext::default()
        };
        context
            .granted_permissions
            .insert(ActionPermission::SceneInspect);
        let authorized = authorize_batch(&batch, &context).expect("policy fixture authorized");

        let result = UnavailableBlenderExecutor.execute(&authorized);
        assert_eq!(result, Err(ExecutionError::ConnectorUnavailable));
        assert_eq!(
            result.expect_err("no Blender action executed").code(),
            "connector_unavailable"
        );
    }

    #[test]
    fn parser_never_returns_partial_validated_batches() {
        let mut invalid = create_action();
        invalid["parameters"]["scale"] = json!([1.0, -1.0, 1.0]);
        let candidate = valid_candidate(vec![action(GET_SCENE_SUMMARY_ID, json!({})), invalid]);
        assert!(validate_candidate_value(&candidate).is_err());
    }
}
