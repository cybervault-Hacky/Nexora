use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Limits and constants
// ---------------------------------------------------------------------------

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_CONVERSATIONS: usize = 500;
pub const MAX_MESSAGES_PER_CONVERSATION: usize = 2000;
pub const MAX_MESSAGE_CHARS: usize = 100_000;
pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_TITLE_DISPLAY: usize = 60;
/// Conversations file name inside app_config_dir
pub const CONVERSATIONS_FILE: &str = "conversations.json";
/// Total payload guard: 10 MiB of JSON per save
pub const MAX_TOTAL_BYTES: usize = 10 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryError {
    pub code: &'static str,
    pub message: String,
}

pub type HistoryResult<T> = Result<T, HistoryError>;

impl HistoryError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new("not_found", msg)
    }
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new("invalid_request", msg)
    }
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::new("storage", msg)
    }
    pub fn unsupported_version(msg: impl Into<String>) -> Self {
        Self::new("unsupported_version", msg)
    }
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for HistoryError {}

// ---------------------------------------------------------------------------
// Persisted types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: u64,
    /// "complete" | "stopped" | "error" — streaming is never persisted as such
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConversation {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<StoredMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationsFile {
    pub version: u32,
    pub conversations: Vec<StoredConversation>,
}

impl Default for ConversationsFile {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            conversations: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

pub fn is_valid_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    // Allow alphanumeric, dash, underscore — covers UUID and fallbackId
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn sanitize_title(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "New conversation".to_string();
    }
    let mut title: String = trimmed.chars().take(MAX_TITLE_CHARS).collect();
    // Collapse newlines to spaces for display
    title = title.replace('\n', " ").replace('\r', " ");
    // Collapse multiple spaces
    let mut collapsed = String::new();
    let mut last_space = false;
    for c in title.chars() {
        if c == ' ' {
            if !last_space {
                collapsed.push(' ');
            }
            last_space = true;
        } else {
            collapsed.push(c);
            last_space = false;
        }
    }
    let final_title = collapsed.trim().to_string();
    if final_title.is_empty() {
        "New conversation".to_string()
    } else {
        final_title
    }
}

pub fn is_valid_role(role: &str) -> bool {
    matches!(role, "user" | "assistant")
}

pub fn validate_message(msg: &StoredMessage) -> HistoryResult<()> {
    if !is_valid_id(&msg.id) {
        return Err(HistoryError::invalid_request(format!(
            "Invalid message id: {}",
            msg.id
        )));
    }
    if !is_valid_role(&msg.role) {
        return Err(HistoryError::invalid_request(format!(
            "Invalid role: {}",
            msg.role
        )));
    }
    let char_count = msg.content.chars().count();
    if char_count > MAX_MESSAGE_CHARS {
        return Err(HistoryError::invalid_request(format!(
            "Message {} is too large ({} chars, max {})",
            msg.id, char_count, MAX_MESSAGE_CHARS
        )));
    }
    if let Some(state) = &msg.state {
        if !matches!(state.as_str(), "complete" | "stopped" | "error" | "streaming") {
            return Err(HistoryError::invalid_request(format!(
                "Invalid message state: {}",
                state
            )));
        }
    }
    if let Some(failure) = &msg.failure {
        if failure.chars().count() > 2000 {
            return Err(HistoryError::invalid_request(
                "Failure message too long",
            ));
        }
    }
    Ok(())
}

pub fn validate_conversation(conv: &StoredConversation) -> HistoryResult<()> {
    if !is_valid_id(&conv.id) {
        return Err(HistoryError::invalid_request(format!(
            "Invalid conversation id: {}",
            conv.id
        )));
    }
    if conv.title.chars().count() > MAX_TITLE_CHARS {
        return Err(HistoryError::invalid_request("Title too long"));
    }
    if conv.messages.len() > MAX_MESSAGES_PER_CONVERSATION {
        return Err(HistoryError::invalid_request(format!(
            "Too many messages: {} (max {})",
            conv.messages.len(),
            MAX_MESSAGES_PER_CONVERSATION
        )));
    }
    for msg in &conv.messages {
        validate_message(msg)?;
    }
    // updated_at should not be before created_at (allow equal)
    if conv.updated_at < conv.created_at {
        return Err(HistoryError::invalid_request(
            "updatedAt cannot be before createdAt",
        ));
    }
    Ok(())
}

pub fn derive_preview(messages: &[StoredMessage]) -> String {
    // Preview: last non-empty message content, first line, truncated to 120 chars
    if messages.is_empty() {
        return String::new();
    }
    for msg in messages.iter().rev() {
        let trimmed = msg.content.trim();
        if !trimmed.is_empty() {
            let first_line = trimmed.lines().next().unwrap_or("").trim();
            let preview: String = first_line.chars().take(120).collect();
            if !preview.is_empty() {
                return preview;
            }
        }
    }
    // fallback to first message
    messages
        .first()
        .map(|m| m.content.chars().take(120).collect())
        .unwrap_or_default()
}

pub fn to_summary(conv: &StoredConversation) -> ConversationSummary {
    ConversationSummary {
        id: conv.id.clone(),
        title: sanitize_title(&conv.title),
        created_at: conv.created_at,
        updated_at: conv.updated_at,
        message_count: conv.messages.len(),
        preview: derive_preview(&conv.messages),
    }
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// File I/O with atomic write and lenient loading
// ---------------------------------------------------------------------------

fn conversations_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CONVERSATIONS_FILE)
}

fn tmp_path_for(target: &Path) -> PathBuf {
    // Use a random suffix to avoid collisions if multiple saves overlap
    let mut rand_bytes = [0u8; 6];
    let _ = getrandom::getrandom(&mut rand_bytes);
    let suffix: String = rand_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("conversations.json");
    target.with_file_name(format!("{}.tmp.{}", file_name, suffix))
}

/// Atomically writes `content` to `path` via temp file + rename.
fn atomic_write(path: &Path, content: &[u8]) -> HistoryResult<()> {
    if content.len() > MAX_TOTAL_BYTES {
        return Err(HistoryError::invalid_request(format!(
            "Payload too large: {} bytes (max {})",
            content.len(),
            MAX_TOTAL_BYTES
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            HistoryError::storage(format!("Could not create conversations folder: {}", e))
        })?;
    }
    let tmp = tmp_path_for(path);
    fs::write(&tmp, content).map_err(|e| {
        HistoryError::storage(format!("Could not write conversations file: {}", e))
    })?;
    // On Unix, rename is atomic when same filesystem. On Windows it replaces.
    fs::rename(&tmp, path).map_err(|e| {
        // Try to clean up tmp
        let _ = fs::remove_file(&tmp);
        HistoryError::storage(format!("Could not finalize conversations file: {}", e))
    })?;
    Ok(())
}

/// Leniently loads conversations file, skipping malformed entries.
/// Returns empty if file missing. Errors only on unsupported version or total corruption that cannot be recovered.
pub fn load_file(config_dir: &Path) -> HistoryResult<Vec<StoredConversation>> {
    let path = conversations_path(config_dir);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(HistoryError::storage(format!(
                "Could not read conversations file: {}",
                e
            )))
        }
    };

    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    // First try to parse as versioned file
    let parsed: Result<ConversationsFile, _> = serde_json::from_str(&text);
    if let Ok(file) = parsed {
        if file.version > CURRENT_SCHEMA_VERSION {
            return Err(HistoryError::unsupported_version(format!(
                "Unsupported storage version {} (current is {}). Please update Nexora.",
                file.version, CURRENT_SCHEMA_VERSION
            )));
        }
        // Migrate if needed (currently v1 only, so just validate and filter)
        let mut valid = Vec::new();
        for mut conv in file.conversations {
            // Migration: ensure title sanitized, timestamps sensible
            conv.title = sanitize_title(&conv.title);
            if conv.updated_at == 0 {
                conv.updated_at = conv.created_at;
            }
            // Coerce streaming state to complete on load (should not be persisted, but handle)
            for msg in &mut conv.messages {
                if msg.state.as_deref() == Some("streaming") {
                    if msg.content.trim().is_empty() {
                        msg.state = Some("stopped".to_string());
                    } else {
                        msg.state = Some("complete".to_string());
                    }
                }
            }
            if validate_conversation(&conv).is_ok() {
                valid.push(conv);
            } else {
                // Skip malformed entry, do not delete whole history
                eprintln!("Skipping malformed conversation {}", conv.id);
            }
        }
        // Sort by updated_at descending for consistency, but preserve original order if needed
        // We'll keep file order; list command will sort.
        return Ok(valid);
    }

    // Fallback: try to parse as bare array (legacy before version field)
    let legacy: Result<Vec<StoredConversation>, _> = serde_json::from_str(&text);
    if let Ok(convs) = legacy {
        let mut valid = Vec::new();
        for mut conv in convs {
            conv.title = sanitize_title(&conv.title);
            if conv.updated_at == 0 {
                conv.updated_at = conv.created_at;
            }
            if validate_conversation(&conv).is_ok() {
                valid.push(conv);
            }
        }
        return Ok(valid);
    }

    // If we get here, file is malformed but not empty — treat as storage error
    // but do not delete. Return error so UI can show it.
    Err(HistoryError::storage(
        "Conversations file is corrupted and could not be parsed.",
    ))
}

pub fn save_file(config_dir: &Path, conversations: &[StoredConversation]) -> HistoryResult<()> {
    if conversations.len() > MAX_CONVERSATIONS {
        return Err(HistoryError::invalid_request(format!(
            "Too many conversations: {} (max {})",
            conversations.len(),
            MAX_CONVERSATIONS
        )));
    }
    let file = ConversationsFile {
        version: CURRENT_SCHEMA_VERSION,
        conversations: conversations.to_vec(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(|e| {
        HistoryError::storage(format!("Could not serialize conversations: {}", e))
    })?;
    atomic_write(&conversations_path(config_dir), text.as_bytes())
}

// ---------------------------------------------------------------------------
// In-memory store with Mutex
// ---------------------------------------------------------------------------

pub struct HistoryStore {
    config_dir: PathBuf,
    // In-memory cache, loaded at startup, kept in sync with file
    conversations: Mutex<Vec<StoredConversation>>,
}

impl HistoryStore {
    pub fn new(config_dir: PathBuf) -> Self {
        let conversations = load_file(&config_dir).unwrap_or_else(|e| {
            eprintln!("Failed to load conversations: {} — starting empty", e);
            Vec::new()
        });
        Self {
            config_dir,
            conversations: Mutex::new(conversations),
        }
    }

    // For tests: create with custom path without reading existing file
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            config_dir: path,
            conversations: Mutex::new(Vec::new()),
        }
    }

    pub fn list(&self) -> HistoryResult<Vec<ConversationSummary>> {
        let guard = self.conversations.lock().map_err(|_| {
            HistoryError::storage("Conversation storage is unavailable")
        })?;
        let mut summaries: Vec<ConversationSummary> = guard.iter().map(to_summary).collect();
        // Most recent first
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    pub fn load(&self, id: &str) -> HistoryResult<StoredConversation> {
        if !is_valid_id(id) {
            return Err(HistoryError::invalid_request("Invalid conversation id"));
        }
        let guard = self.conversations.lock().map_err(|_| {
            HistoryError::storage("Conversation storage is unavailable")
        })?;
        guard
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| HistoryError::not_found(format!("Conversation {} not found", id)))
    }

    pub fn create(&self, title: Option<String>) -> HistoryResult<StoredConversation> {
        let now = now_millis();
        // Generate a stable id server-side
        let id = generate_id();
        let title = sanitize_title(&title.unwrap_or_else(|| "New conversation".to_string()));
        let conv = StoredConversation {
            id: id.clone(),
            title,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        };
        validate_conversation(&conv)?;
        {
            let mut guard = self.conversations.lock().map_err(|_| {
                HistoryError::storage("Conversation storage is unavailable")
            })?;
            if guard.len() >= MAX_CONVERSATIONS {
                return Err(HistoryError::invalid_request(format!(
                    "Too many conversations (max {})",
                    MAX_CONVERSATIONS
                )));
            }
            if guard.iter().any(|c| c.id == id) {
                return Err(HistoryError::storage("ID collision, try again"));
            }
            guard.push(conv.clone());
            // Persist
            save_file(&self.config_dir, &guard)?;
        }
        Ok(conv)
    }

    pub fn create_with_id(&self, id: String, title: Option<String>) -> HistoryResult<StoredConversation> {
        if !is_valid_id(&id) {
            return Err(HistoryError::invalid_request("Invalid conversation id"));
        }
        let now = now_millis();
        let title = sanitize_title(&title.unwrap_or_else(|| "New conversation".to_string()));
        let conv = StoredConversation {
            id: id.clone(),
            title,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        };
        validate_conversation(&conv)?;
        {
            let mut guard = self.conversations.lock().map_err(|_| {
                HistoryError::storage("Conversation storage is unavailable")
            })?;
            if guard.len() >= MAX_CONVERSATIONS {
                return Err(HistoryError::invalid_request(format!(
                    "Too many conversations (max {})",
                    MAX_CONVERSATIONS
                )));
            }
            if guard.iter().any(|c| c.id == id) {
                return Err(HistoryError::invalid_request("Conversation id already exists"));
            }
            guard.push(conv.clone());
            save_file(&self.config_dir, &guard)?;
        }
        Ok(conv)
    }

    pub fn save(&self, mut conversation: StoredConversation) -> HistoryResult<StoredConversation> {
        // Sanitize and validate before acquiring lock
        conversation.title = sanitize_title(&conversation.title);
        // Update timestamp to now
        conversation.updated_at = now_millis();
        // Normalize messages: if any still streaming, coerce
        for msg in &mut conversation.messages {
            if msg.state.as_deref() == Some("streaming") {
                if msg.content.trim().is_empty() {
                    msg.state = Some("stopped".to_string());
                } else {
                    msg.state = Some("complete".to_string());
                }
            }
        }
        validate_conversation(&conversation)?;

        let mut guard = self.conversations.lock().map_err(|_| {
            HistoryError::storage("Conversation storage is unavailable")
        })?;

        // Find existing or push as new
        if let Some(existing) = guard.iter_mut().find(|c| c.id == conversation.id) {
            *existing = conversation.clone();
        } else {
            if guard.len() >= MAX_CONVERSATIONS {
                return Err(HistoryError::invalid_request(format!(
                    "Too many conversations (max {})",
                    MAX_CONVERSATIONS
                )));
            }
            guard.push(conversation.clone());
        }

        // Persist atomically
        save_file(&self.config_dir, &guard)?;
        Ok(conversation)
    }

    pub fn rename(&self, id: &str, title: String) -> HistoryResult<ConversationSummary> {
        if !is_valid_id(id) {
            return Err(HistoryError::invalid_request("Invalid conversation id"));
        }
        let sanitized = sanitize_title(&title);
        if sanitized.chars().count() > MAX_TITLE_CHARS {
            return Err(HistoryError::invalid_request("Title too long"));
        }
        let mut guard = self.conversations.lock().map_err(|_| {
            HistoryError::storage("Conversation storage is unavailable")
        })?;
        let conv = guard
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| HistoryError::not_found(format!("Conversation {} not found", id)))?;
        conv.title = sanitized;
        conv.updated_at = now_millis();
        let summary = to_summary(conv);
        save_file(&self.config_dir, &guard)?;
        Ok(summary)
    }

    pub fn delete(&self, id: &str) -> HistoryResult<bool> {
        if !is_valid_id(id) {
            return Err(HistoryError::invalid_request("Invalid conversation id"));
        }
        let mut guard = self.conversations.lock().map_err(|_| {
            HistoryError::storage("Conversation storage is unavailable")
        })?;
        let original_len = guard.len();
        guard.retain(|c| c.id != id);
        if guard.len() == original_len {
            return Err(HistoryError::not_found(format!("Conversation {} not found", id)));
        }
        // Only the selected conversation is removed; guard now has filtered list
        save_file(&self.config_dir, &guard)?;
        Ok(true)
    }

    /// For testing: get raw path
    pub fn path(&self) -> PathBuf {
        conversations_path(&self.config_dir)
    }

    /// For testing: clear all
    pub fn clear(&self) -> HistoryResult<()> {
        let mut guard = self.conversations.lock().map_err(|_| {
            HistoryError::storage("Conversation storage is unavailable")
        })?;
        guard.clear();
        save_file(&self.config_dir, &guard)?;
        Ok(())
    }
}

fn generate_id() -> String {
    // Use getrandom to generate a UUID-like id, or fallback to hex
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_ok() {
        // Format as hex with dashes like UUID v4-ish (not strictly spec, but stable)
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15]
        )
    } else {
        // Fallback
        format!("id-{}-{}", now_millis(), bytes[0])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn temp_store() -> (HistoryStore, TempDir) {
        let dir = TempDir::new().expect("temp dir");
        let store = HistoryStore::new(dir.path().to_path_buf());
        (store, dir)
    }

    fn make_message(id: &str, role: &str, content: &str) -> StoredMessage {
        StoredMessage {
            id: id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now_millis(),
            state: Some("complete".to_string()),
            failure: None,
        }
    }

    #[test]
    fn create_and_list() {
        let (store, _dir) = temp_store();
        let conv = store.create(Some("Test chat".to_string())).expect("create");
        assert!(is_valid_id(&conv.id));
        assert_eq!(conv.title, "Test chat");
        assert_eq!(conv.messages.len(), 0);

        let list = store.list().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, conv.id);
        assert_eq!(list[0].message_count, 0);
    }

    #[test]
    fn save_and_load() {
        let (store, _dir) = temp_store();
        let mut conv = store.create(Some("Hello".to_string())).expect("create");
        conv.messages.push(make_message("msg-1", "user", "Hello world"));
        conv.messages
            .push(make_message("msg-2", "assistant", "Hi there!"));
        let saved = store.save(conv.clone()).expect("save");
        assert_eq!(saved.messages.len(), 2);

        let loaded = store.load(&saved.id).expect("load");
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "Hello world");
        assert_eq!(loaded.messages[1].content, "Hi there!");
    }

    #[test]
    fn multiple_independent_conversations() {
        let (store, _dir) = temp_store();
        let a = store.create(Some("A".to_string())).expect("a");
        let b = store.create(Some("B".to_string())).expect("b");
        let c = store.create(Some("C".to_string())).expect("c");

        // Save distinct messages to each
        let mut ca = store.load(&a.id).expect("load a");
        ca.messages.push(make_message("m1", "user", "Message for A"));
        store.save(ca).expect("save a");

        let mut cb = store.load(&b.id).expect("load b");
        cb.messages.push(make_message("m2", "user", "Message for B"));
        store.save(cb).expect("save b");

        let la = store.load(&a.id).expect("load a again");
        let lb = store.load(&b.id).expect("load b again");
        let lc = store.load(&c.id).expect("load c again");

        assert_eq!(la.messages.len(), 1);
        assert_eq!(la.messages[0].content, "Message for A");
        assert_eq!(lb.messages.len(), 1);
        assert_eq!(lb.messages[0].content, "Message for B");
        assert_eq!(lc.messages.len(), 0);

        let list = store.list().expect("list");
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn rename() {
        let (store, _dir) = temp_store();
        let conv = store.create(Some("Old".to_string())).expect("create");
        let renamed = store.rename(&conv.id, "New title".to_string()).expect("rename");
        assert_eq!(renamed.title, "New title");

        let loaded = store.load(&conv.id).expect("load");
        assert_eq!(loaded.title, "New title");
    }

    #[test]
    fn delete() {
        let (store, _dir) = temp_store();
        let a = store.create(Some("A".to_string())).expect("a");
        let b = store.create(Some("B".to_string())).expect("b");

        store.delete(&a.id).expect("delete");
        let list = store.list().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, b.id);

        // Deleting only selected conversation, not all
        assert!(store.load(&b.id).is_ok());
        assert!(store.load(&a.id).is_err());
    }

    #[test]
    fn delete_nonexistent_fails() {
        let (store, _dir) = temp_store();
        let err = store.delete("nonexistent-id").unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn unicode_multiline_large_messages() {
        let (store, _dir) = temp_store();
        let mut conv = store.create(Some("Unicode test".to_string())).expect("create");

        // Unicode
        conv.messages.push(make_message(
            "u1",
            "user",
            "Hello 🌍 Привет こんにちは",
        ));
        // Multiline with code
        conv.messages.push(make_message(
            "u2",
            "assistant",
            "```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\nInline `code` here.",
        ));
        // Large but permitted (10k chars)
        let large = "A".repeat(10_000);
        conv.messages.push(make_message("u3", "user", &large));

        store.save(conv.clone()).expect("save");
        let loaded = store.load(&conv.id).expect("load");
        assert_eq!(loaded.messages[0].content, "Hello 🌍 Привет こんにちは");
        assert!(loaded.messages[1].content.contains("fn main()"));
        assert!(loaded.messages[1].content.contains("`code`"));
        assert_eq!(loaded.messages[2].content.len(), 10_000);
    }

    #[test]
    fn empty_conversation() {
        let (store, _dir) = temp_store();
        let conv = store.create(Some("Empty".to_string())).expect("create");
        // Save with no messages explicitly
        let saved = store.save(conv.clone()).expect("save");
        assert_eq!(saved.messages.len(), 0);

        let loaded = store.load(&conv.id).expect("load");
        assert_eq!(loaded.messages.len(), 0);
    }

    #[test]
    fn missing_or_malformed_records() {
        let (store, dir) = temp_store();
        // Create a valid one
        let conv = store.create(Some("Good".to_string())).expect("create");

        // Corrupt file with one good and one bad entry (invalid role)
        let path = dir.path().join(CONVERSATIONS_FILE);
        let text = fs::read_to_string(&path).expect("read");
        let mut file: ConversationsFile = serde_json::from_str(&text).expect("parse");

        // Add a malformed conversation with invalid id
        file.conversations.push(StoredConversation {
            id: "".to_string(), // invalid empty id
            title: "Bad".to_string(),
            created_at: now_millis(),
            updated_at: now_millis(),
            messages: vec![],
        });
        // Add a second malformed with invalid role message
        file.conversations.push(StoredConversation {
            id: "valid-id-123".to_string(),
            title: "Also bad".to_string(),
            created_at: now_millis(),
            updated_at: now_millis(),
            messages: vec![StoredMessage {
                id: "m1".to_string(),
                role: "tool".to_string(), // invalid role
                content: "hi".to_string(),
                created_at: now_millis(),
                state: None,
                failure: None,
            }],
        });

        let corrupted = serde_json::to_string_pretty(&file).expect("ser");
        fs::write(&path, corrupted).expect("write corrupt");

        // Load should skip malformed and return only valid
        let loaded = load_file(dir.path()).expect("lenient load");
        // Should have only the original good one, bad ones skipped
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, conv.id);
    }

    #[test]
    fn unsupported_schema_version() {
        let dir = TempDir::new().expect("temp");
        let path = dir.path().join(CONVERSATIONS_FILE);
        let file = ConversationsFile {
            version: 999,
            conversations: vec![],
        };
        let text = serde_json::to_string_pretty(&file).expect("ser");
        fs::write(&path, text).expect("write");

        let err = load_file(dir.path()).unwrap_err();
        assert_eq!(err.code, "unsupported_version");
    }

    #[test]
    fn atomic_save_no_corruption() {
        let (store, dir) = temp_store();
        let mut conv = store.create(Some("Test".to_string())).expect("create");
        conv.messages.push(make_message("m1", "user", "Hello"));
        store.save(conv).expect("save");

        let path = dir.path().join(CONVERSATIONS_FILE);
        assert!(path.exists());
        let content = fs::read_to_string(&path).expect("read");
        // Should be valid JSON
        let parsed: ConversationsFile = serde_json::from_str(&content).expect("valid json");
        assert_eq!(parsed.version, CURRENT_SCHEMA_VERSION);
        assert_eq!(parsed.conversations.len(), 1);

        // No tmp files left behind
        let entries: Vec<_> = fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        for name in &entries {
            assert!(
                !name.contains(".tmp."),
                "tmp file left behind: {}",
                name
            );
        }
    }

    #[test]
    fn interrupted_save_does_not_corrupt() {
        // Simulate interrupted write by writing a tmp file manually and ensuring original remains valid
        let (store, dir) = temp_store();
        let conv = store.create(Some("Original".to_string())).expect("create");
        let path = dir.path().join(CONVERSATIONS_FILE);
        let original = fs::read_to_string(&path).expect("original read");

        // Write a partial tmp file as if interrupted
        let tmp = dir.path().join("conversations.json.tmp.test123");
        fs::write(&tmp, "{ incomplete json").expect("tmp write");

        // Original should still be readable and valid
        let loaded = load_file(dir.path()).expect("load after interrupted");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, conv.id);

        // Cleanup tmp
        let _ = fs::remove_file(tmp);
        // Original content unchanged
        let after = fs::read_to_string(&path).expect("after read");
        assert_eq!(original, after);

        // Now save a new conversation — should overwrite cleanly
        let conv2 = store.create(Some("Second".to_string())).expect("create2");
        assert!(store.load(&conv2.id).is_ok());
        assert!(store.load(&conv.id).is_ok());
    }

    #[test]
    fn id_validation() {
        assert!(is_valid_id("abc-123_def"));
        assert!(is_valid_id("550e8400-e29b-41d4-a716-446655440000")); // UUID
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("has space"));
        assert!(!is_valid_id("has/slash"));
        assert!(!is_valid_id("has.dot"));
        assert!(!is_valid_id(&"a".repeat(129))); // too long
    }

    #[test]
    fn size_limits() {
        let (store, _dir) = temp_store();
        let mut conv = store.create(Some("Test".to_string())).expect("create");

        // Too many messages
        conv.messages = (0..MAX_MESSAGES_PER_CONVERSATION + 1)
            .map(|i| make_message(&format!("m{}", i), "user", "hi"))
            .collect();
        let err = store.save(conv).unwrap_err();
        assert_eq!(err.code, "invalid_request");

        // Too large message
        let (store2, _dir2) = temp_store();
        let mut c2 = store2.create(Some("Test2".to_string())).expect("create");
        let huge = "a".repeat(MAX_MESSAGE_CHARS + 1);
        c2.messages.push(make_message("m1", "user", &huge));
        let err2 = store2.save(c2).unwrap_err();
        assert_eq!(err2.code, "invalid_request");
    }

    #[test]
    fn isolation_between_ids() {
        let (store, _dir) = temp_store();
        let a = store.create(Some("A".to_string())).expect("a");
        let b = store.create(Some("B".to_string())).expect("b");

        // Modify A, ensure B unchanged
        let mut ca = store.load(&a.id).expect("load a");
        ca.messages.push(make_message("m1", "user", "Only A"));
        store.save(ca).expect("save a");

        let lb = store.load(&b.id).expect("load b");
        assert_eq!(lb.messages.len(), 0);

        // Rename A, ensure B title unchanged
        store.rename(&a.id, "A renamed".to_string()).expect("rename");
        let lb2 = store.load(&b.id).expect("load b again");
        assert_eq!(lb2.title, "B");
    }

    #[test]
    fn title_sanitization() {
        assert_eq!(sanitize_title("  hello  "), "hello");
        assert_eq!(sanitize_title(""), "New conversation");
        assert_eq!(sanitize_title("   "), "New conversation");
        assert_eq!(sanitize_title("a\nb"), "a b");
        // Long title truncated to 200
        let long = "a".repeat(300);
        assert_eq!(sanitize_title(&long).len(), 200);
    }

    #[test]
    fn preview_derivation() {
        let msgs = vec![
            make_message("1", "user", "First prompt"),
            make_message("2", "assistant", "Answer\nsecond line"),
        ];
        let preview = derive_preview(&msgs);
        assert_eq!(preview, "Answer");

        let empty: Vec<StoredMessage> = vec![];
        assert_eq!(derive_preview(&empty), "");
    }

    #[test]
    fn corrupted_file_returns_storage_error_not_panic() {
        let dir = TempDir::new().expect("temp");
        let path = dir.path().join(CONVERSATIONS_FILE);
        fs::write(&path, "not json at all {{{").expect("write bad");

        let err = load_file(dir.path()).unwrap_err();
        assert_eq!(err.code, "storage");
    }

    #[test]
    fn create_with_id_validation() {
        let (store, _dir) = temp_store();
        let err = store
            .create_with_id("bad id with spaces".to_string(), Some("t".to_string()))
            .unwrap_err();
        assert_eq!(err.code, "invalid_request");

        let ok = store
            .create_with_id("valid-id_123".to_string(), Some("My chat".to_string()))
            .expect("valid");
        assert_eq!(ok.id, "valid-id_123");
    }

    #[test]
    fn save_coerces_streaming_state() {
        let (store, _dir) = temp_store();
        let mut conv = store.create(Some("Test".to_string())).expect("create");
        conv.messages.push(StoredMessage {
            id: "m1".to_string(),
            role: "assistant".to_string(),
            content: "partial".to_string(),
            created_at: now_millis(),
            state: Some("streaming".to_string()),
            failure: None,
        });
        let saved = store.save(conv).expect("save");
        assert_eq!(saved.messages[0].state.as_deref(), Some("complete"));

        // Empty streaming becomes stopped
        let mut conv2 = store.create(Some("Test2".to_string())).expect("create2");
        conv2.messages.push(StoredMessage {
            id: "m2".to_string(),
            role: "assistant".to_string(),
            content: "".to_string(),
            created_at: now_millis(),
            state: Some("streaming".to_string()),
            failure: None,
        });
        let saved2 = store.save(conv2).expect("save2");
        assert_eq!(saved2.messages[0].state.as_deref(), Some("stopped"));
    }

    #[test]
    fn delete_only_selected() {
        let (store, _dir) = temp_store();
        let ids: Vec<String> = (0..5)
            .map(|i| {
                store
                    .create(Some(format!("Chat {}", i)))
                    .expect("create")
                    .id
            })
            .collect();

        let to_delete = ids[2].clone();
        store.delete(&to_delete).expect("delete");

        let list = store.list().expect("list");
        assert_eq!(list.len(), 4);
        assert!(!list.iter().any(|s| s.id == to_delete));
        for id in &ids {
            if id != &to_delete {
                assert!(store.load(id).is_ok(), "should still exist: {}", id);
            }
        }
    }

    #[test]
    fn storage_permission_failure_handled() {
        // This is hard to test portably without root; we test that save_file returns error on invalid path
        // Create a store with a file path that is actually a directory to cause write failure
        let dir = TempDir::new().expect("temp");
        let file_path = dir.path().join(CONVERSATIONS_FILE);
        // Make the target path a directory to cause write failure
        fs::create_dir_all(&file_path).expect("mkdir as file");

        let store = HistoryStore::new(dir.path().to_path_buf());
        // Try to create — should fail gracefully with storage error
        let result = store.create(Some("Test".to_string()));
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.code, "storage");
        }
    }
}
