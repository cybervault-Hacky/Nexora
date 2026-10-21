pub mod commands;
pub mod validator;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LicenceTier {
    Community,
    Pro,
    Team,
    Enterprise,
}

impl Default for LicenceTier {
    fn default() -> Self {
        LicenceTier::Community
    }
}

impl std::fmt::Display for LicenceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenceTier::Community => write!(f, "Community"),
            LicenceTier::Pro => write!(f, "Pro"),
            LicenceTier::Team => write!(f, "Team"),
            LicenceTier::Enterprise => write!(f, "Enterprise"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenceStatus {
    pub tier: String,
    pub plan_name: String,
    pub active: bool,
    pub licence_key: Option<String>,
    pub licensee: Option<String>,
    pub expires_at: Option<i64>,
    pub features: Vec<String>,
}

impl Default for LicenceStatus {
    fn default() -> Self {
        Self {
            tier: "community".to_string(),
            plan_name: "Community Edition".to_string(),
            active: true,
            licence_key: None,
            licensee: None,
            expires_at: None,
            features: vec![
                "daily_local_inference".to_string(),
                "gguf_model_execution".to_string(),
                "offline_ai_chat".to_string(),
                "workspace_code_intelligence".to_string(),
                "single_file_and_batch_patches".to_string(),
                "ast_symbol_indexing".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateLicencePayload {
    pub licence_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenceError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for LicenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for LicenceError {}
