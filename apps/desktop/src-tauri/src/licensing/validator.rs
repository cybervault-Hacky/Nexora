use super::{LicenceError, LicenceStatus, LicenceTier};
use sha2::{Digest, Sha256};

const LICENCE_SALT: &[u8] = b"NEXORA_OFFLINE_LICENCE_VERIFIER_SALT_v1";

pub struct ValidatedLicence {
    pub tier: LicenceTier,
    pub licensee: String,
    pub expires_at: Option<i64>,
    pub features: Vec<String>,
}

/// Computes the expected 8-hex-character checksum for a licence key prefix.
pub fn compute_licence_checksum(prefix: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(LICENCE_SALT);
    let hash = hasher.finalize();
    format!(
        "{:02X}{:02X}{:02X}{:02X}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

/// Validates an offline licence key without requiring network connectivity.
pub fn validate_licence_key(raw_key: &str) -> Result<ValidatedLicence, LicenceError> {
    let key = raw_key.trim().to_uppercase();
    let parts: Vec<&str> = key.split('-').collect();

    if parts.len() != 4 || parts[0] != "NEXORA" {
        return Err(LicenceError {
            code: "invalid_format".to_string(),
            message: "Licence key format must match NEXORA-<TIER>-<BODY>-<CHECKSUM>.".to_string(),
        });
    }

    let tier_str = parts[1];
    let body = parts[2];
    let provided_checksum = parts[3];

    let prefix = format!("NEXORA-{}-{}", tier_str, body);
    let expected_checksum = compute_licence_checksum(&prefix);

    if provided_checksum != expected_checksum {
        return Err(LicenceError {
            code: "invalid_checksum".to_string(),
            message: "Licence key signature or checksum verification failed.".to_string(),
        });
    }

    let tier = match tier_str {
        "PRO" => LicenceTier::Pro,
        "TEAM" => LicenceTier::Team,
        "ENT" | "ENTERPRISE" => LicenceTier::Enterprise,
        _ => {
            return Err(LicenceError {
                code: "unsupported_tier".to_string(),
                message: format!("Unrecognised licence tier: {}", tier_str),
            })
        }
    };

    let body_parts: Vec<&str> = body.split('_').collect();
    let exp_part = body_parts.get(0).copied().unwrap_or("PERPETUAL");
    let licensee = body_parts
        .get(1)
        .copied()
        .unwrap_or("REGISTERED_DEVELOPER")
        .to_string();

    let expires_at = if exp_part == "PERPETUAL" {
        None
    } else if let Some(stripped) = exp_part.strip_prefix("EXP") {
        match stripped.parse::<i64>() {
            Ok(ts) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                if ts < now {
                    return Err(LicenceError {
                        code: "licence_expired".to_string(),
                        message: "The provided licence key has expired.".to_string(),
                    });
                }
                Some(ts)
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let mut features = vec![
        "daily_local_inference".to_string(),
        "gguf_model_execution".to_string(),
        "offline_ai_chat".to_string(),
        "workspace_code_intelligence".to_string(),
        "single_file_and_batch_patches".to_string(),
        "ast_symbol_indexing".to_string(),
        "commercial_use_rights".to_string(),
        "deep_dependency_graph".to_string(),
    ];

    if tier == LicenceTier::Team || tier == LicenceTier::Enterprise {
        features.push("multi_seat_workspaces".to_string());
        features.push("founder_control_team_audit".to_string());
    }

    if tier == LicenceTier::Enterprise {
        features.push("priority_security_patches".to_string());
        features.push("custom_telemetry_connectors".to_string());
    }

    Ok(ValidatedLicence {
        tier,
        licensee,
        expires_at,
        features,
    })
}

pub fn to_licence_status(
    validated: Option<&ValidatedLicence>,
    raw_key: Option<String>,
) -> LicenceStatus {
    match validated {
        Some(v) => {
            let plan_name = match v.tier {
                LicenceTier::Community => "Community Edition".to_string(),
                LicenceTier::Pro => "Developer Pro Edition".to_string(),
                LicenceTier::Team => "Team Commercial Edition".to_string(),
                LicenceTier::Enterprise => "Enterprise Suite".to_string(),
            };

            LicenceStatus {
                tier: v.tier.to_string().to_lowercase(),
                plan_name,
                active: true,
                licence_key: raw_key,
                licensee: Some(v.licensee.clone()),
                expires_at: v.expires_at,
                features: v.features.clone(),
            }
        }
        None => LicenceStatus::default(),
    }
}
