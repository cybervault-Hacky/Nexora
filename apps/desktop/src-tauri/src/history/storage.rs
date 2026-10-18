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
pub const MAX_MESSAGE_CHARS: usize = 400_000;
pub const MAX_TITLE_CHARS: usize = 200;

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
    title = title.replace(['\n', '\r'], " ");
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
        return Err(HistoryError::invalid_request("Invalid message identifier."));
    }
    if !is_valid_role(&msg.role) {
        return Err(HistoryError::invalid_request("Invalid message role."));
    }
    let char_count = msg.content.chars().count();
    if char_count > MAX_MESSAGE_CHARS {
        return Err(HistoryError::invalid_request(format!(
            "Message {} is too large ({} chars, max {})",
            msg.id, char_count, MAX_MESSAGE_CHARS
        )));
    }
    if let Some(state) = &msg.state {
        if !matches!(
            state.as_str(),
            "complete" | "stopped" | "error" | "streaming"
        ) {
            return Err(HistoryError::invalid_request("Invalid message state."));
        }
    }
    if let Some(failure) = &msg.failure {
        if failure.chars().count() > 2000 {
            return Err(HistoryError::invalid_request("Failure message too long"));
        }
    }
    Ok(())
}

pub fn validate_conversation(conv: &StoredConversation) -> HistoryResult<()> {
    if !is_valid_id(&conv.id) {
        return Err(HistoryError::invalid_request(
            "Invalid conversation identifier.",
        ));
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
    let mut ids = std::collections::HashSet::new();
    for msg in &conv.messages {
        validate_message(msg)?;
        if msg.created_at > 8_640_000_000_000_000 || !ids.insert(&msg.id) {
            return Err(HistoryError::invalid_request(
                "Invalid message timestamp or duplicate identifier.",
            ));
        }
    }
    if conv.created_at > 8_640_000_000_000_000 || conv.updated_at > 8_640_000_000_000_000 {
        return Err(HistoryError::invalid_request(
            "Invalid conversation timestamp.",
        ));
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

/// Same-directory temporary file, synced before atomic replacement on Windows/Unix.
fn atomic_write(path: &Path, content: &[u8]) -> HistoryResult<()> {
    use std::io::Write;
    if content.len() > MAX_TOTAL_BYTES {
        return Err(HistoryError::invalid_request(
            "History exceeds the 10 MiB storage limit.",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| HistoryError::storage("Invalid storage directory."))?;
    fs::create_dir_all(parent)
        .map_err(|_| HistoryError::storage("Could not create history directory."))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| HistoryError::storage("Could not create temporary history file."))?;
    tmp.write_all(content)
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|_| {
            HistoryError::storage("Could not write history. Check disk space and permissions.")
        })?;
    tmp.persist(path)
        .map_err(|_| HistoryError::storage("Could not replace history file."))?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| {
            HistoryError::storage("History replacement could not be confirmed durable.")
        })?;
    Ok(())
}

/// Fail closed: malformed/unknown data stays on disk and blocks writes.
/// Version zero and the legacy bare array migrate in memory; the next successful
/// edit writes version one. No prior shipping version persisted conversations.
pub fn load_file(config_dir: &Path) -> HistoryResult<Vec<StoredConversation>> {
    use std::io::Read;
    let file = match fs::File::open(conversations_path(config_dir)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => {
            return Err(HistoryError::storage(
                "Could not read history. Original file preserved.",
            ))
        }
    };
    let mut bytes = Vec::new();
    file.take((MAX_TOTAL_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| HistoryError::storage("Could not read history."))?;
    if bytes.len() > MAX_TOTAL_BYTES {
        return Err(HistoryError::storage(
            "History exceeds the storage limit. Original file preserved.",
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        HistoryError::storage("History is malformed. Original file preserved; writes blocked.")
    })?;
    let conversations = if value.is_array() {
        value
    } else {
        let version = value
            .get("version")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| HistoryError::storage("History has no valid schema version."))?;
        if version > u64::from(CURRENT_SCHEMA_VERSION) {
            return Err(HistoryError::unsupported_version(
                "History requires a newer Nexora. Original file preserved.",
            ));
        }
        value
            .get("conversations")
            .cloned()
            .ok_or_else(|| HistoryError::storage("History records are missing."))?
    };
    let mut records: Vec<StoredConversation> =
        serde_json::from_value(conversations).map_err(|_| {
            HistoryError::storage(
                "A history record is malformed. Original file preserved; writes blocked.",
            )
        })?;
    if records.len() > MAX_CONVERSATIONS {
        return Err(HistoryError::storage("History contains too many records."));
    }
    let mut ids = std::collections::HashSet::new();
    for record in &mut records {
        validate_conversation(record).map_err(|_| {
            HistoryError::storage(
                "A history record is invalid. Original file preserved; writes blocked.",
            )
        })?;
        if !ids.insert(record.id.clone()) {
            return Err(HistoryError::storage(
                "History contains duplicate identifiers.",
            ));
        }
        for message in &mut record.messages {
            if message.state.as_deref() == Some("streaming") {
                message.state = Some("stopped".into());
            }
        }
    }
    Ok(records)
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
    let text = serde_json::to_string_pretty(&file)
        .map_err(|e| HistoryError::storage(format!("Could not serialize conversations: {}", e)))?;
    atomic_write(&conversations_path(config_dir), text.as_bytes())
}

// ---------------------------------------------------------------------------
// In-memory store with Mutex
// ---------------------------------------------------------------------------

pub struct HistoryStore {
    config_dir: PathBuf,
    conversations: Mutex<HistoryResult<Vec<StoredConversation>>>,
}

impl HistoryStore {
    pub fn new(config_dir: PathBuf) -> Self {
        let records = load_file(&config_dir);
        Self {
            config_dir,
            conversations: Mutex::new(records),
        }
    }

    fn read<T>(
        &self,
        action: impl FnOnce(&[StoredConversation]) -> HistoryResult<T>,
    ) -> HistoryResult<T> {
        let guard = self
            .conversations
            .lock()
            .map_err(|_| HistoryError::storage("History unavailable."))?;
        match &*guard {
            Ok(records) => action(records),
            Err(error) => Err(error.clone()),
        }
    }

    /// Commit cache only after disk acknowledges the entire replacement.
    fn change<T>(
        &self,
        action: impl FnOnce(&mut Vec<StoredConversation>) -> HistoryResult<T>,
    ) -> HistoryResult<T> {
        let mut guard = self
            .conversations
            .lock()
            .map_err(|_| HistoryError::storage("History unavailable."))?;
        let mut next = match &*guard {
            Ok(records) => records.clone(),
            Err(error) => return Err(error.clone()),
        };
        let result = action(&mut next)?;
        save_file(&self.config_dir, &next)?;
        *guard = Ok(next);
        Ok(result)
    }

    pub fn list(&self) -> HistoryResult<Vec<ConversationSummary>> {
        self.read(|records| {
            let mut summaries: Vec<_> = records.iter().map(to_summary).collect();
            summaries.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
            Ok(summaries)
        })
    }

    pub fn load(&self, id: &str) -> HistoryResult<StoredConversation> {
        check_id(id)?;
        self.read(|records| {
            records
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .ok_or_else(|| HistoryError::not_found("Conversation not found."))
        })
    }

    pub fn create(&self, title: Option<String>) -> HistoryResult<StoredConversation> {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes)
            .map_err(|_| HistoryError::storage("No secure random source."))?;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        self.create_with_id(id, title)
    }

    pub fn create_with_id(
        &self,
        id: String,
        title: Option<String>,
    ) -> HistoryResult<StoredConversation> {
        check_id(&id)?;
        let title = checked_title(title.as_deref().unwrap_or("New conversation"))?;
        let now = now_millis();
        let conversation = StoredConversation {
            id,
            title,
            created_at: now,
            updated_at: now,
            messages: vec![],
        };
        self.change(|records| {
            if records.len() >= MAX_CONVERSATIONS || records.iter().any(|c| c.id == conversation.id)
            {
                return Err(HistoryError::invalid_request(
                    "History full or identifier already exists.",
                ));
            }
            records.push(conversation.clone());
            Ok(conversation)
        })
    }

    pub fn save(&self, mut conversation: StoredConversation) -> HistoryResult<StoredConversation> {
        validate_conversation(&conversation)?;
        conversation.title = checked_title(&conversation.title)?;
        for msg in &mut conversation.messages {
            if msg.state.as_deref() == Some("streaming") {
                msg.state = Some("stopped".into());
            }
        }
        self.change(|records| {
            if let Some(existing) = records.iter_mut().find(|c| c.id == conversation.id) {
                conversation.created_at = existing.created_at;
                conversation.updated_at = now_millis().max(existing.updated_at.saturating_add(1));
                *existing = conversation.clone();
            } else {
                if records.len() >= MAX_CONVERSATIONS {
                    return Err(HistoryError::invalid_request("History is full."));
                }
                conversation.updated_at = now_millis().max(conversation.created_at);
                records.push(conversation.clone());
            }
            Ok(conversation)
        })
    }

    pub fn rename(&self, id: &str, title: String) -> HistoryResult<ConversationSummary> {
        check_id(id)?;
        let title = checked_title(&title)?;
        self.change(|records| {
            let conv = records
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| HistoryError::not_found("Conversation not found."))?;
            conv.title = title;
            conv.updated_at = now_millis().max(conv.updated_at.saturating_add(1));
            Ok(to_summary(conv))
        })
    }

    pub fn delete(&self, id: &str) -> HistoryResult<bool> {
        check_id(id)?;
        self.change(|records| {
            let index = records
                .iter()
                .position(|c| c.id == id)
                .ok_or_else(|| HistoryError::not_found("Conversation not found."))?;
            records.remove(index);
            Ok(true)
        })
    }
}

fn check_id(id: &str) -> HistoryResult<()> {
    if is_valid_id(id) {
        Ok(())
    } else {
        Err(HistoryError::invalid_request("Invalid identifier."))
    }
}

fn checked_title(title: &str) -> HistoryResult<String> {
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(HistoryError::invalid_request("Title too long."));
    }
    Ok(sanitize_title(title))
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
        conv.messages
            .push(make_message("msg-1", "user", "Hello world"));
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
        ca.messages
            .push(make_message("m1", "user", "Message for A"));
        store.save(ca).expect("save a");

        let mut cb = store.load(&b.id).expect("load b");
        cb.messages
            .push(make_message("m2", "user", "Message for B"));
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
        let renamed = store
            .rename(&conv.id, "New title".to_string())
            .expect("rename");
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
        let mut conv = store
            .create(Some("Unicode test".to_string()))
            .expect("create");

        // Unicode
        conv.messages
            .push(make_message("u1", "user", "Hello 🌍 Привет こんにちは"));
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
        let original = fs::read(&path).expect("original");
        assert!(load_file(dir.path()).is_err());
        let reopened = HistoryStore::new(dir.path().to_path_buf());
        assert!(reopened.create(Some("No overwrite".into())).is_err());
        assert_eq!(fs::read(&path).expect("preserved"), original);
        assert!(is_valid_id(&conv.id));
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
            assert!(!name.contains(".tmp."), "tmp file left behind: {}", name);
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
        store
            .rename(&a.id, "A renamed".to_string())
            .expect("rename");
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
        assert_eq!(saved.messages[0].state.as_deref(), Some("stopped"));

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
    #[test]
    fn failed_write_does_not_change_cache() {
        let (store, dir) = temp_store();
        let conv = store.create(Some("Original".into())).expect("create");
        let path = dir.path().join(CONVERSATIONS_FILE);
        fs::remove_file(&path).expect("remove file");
        fs::create_dir(&path).expect("block replacement");
        assert!(store.rename(&conv.id, "Must not appear".into()).is_err());
        assert_eq!(store.load(&conv.id).expect("cache").title, "Original");
        assert!(store.delete(&conv.id).is_err());
        assert_eq!(store.list().expect("list").len(), 1);
        assert!(store.create(Some("Must not appear".into())).is_err());
        assert_eq!(store.list().expect("list").len(), 1);
    }

    #[test]
    fn migrates_legacy_and_zero_without_rewriting_on_read() {
        let (store, dir) = temp_store();
        let conv = store.create(Some("Legacy".into())).expect("create");
        let path = dir.path().join(CONVERSATIONS_FILE);
        for legacy in [
            serde_json::to_vec(&vec![conv.clone()]).expect("array"),
            serde_json::to_vec(&ConversationsFile {
                version: 0,
                conversations: vec![conv.clone()],
            })
            .expect("v0"),
        ] {
            fs::write(&path, &legacy).expect("write legacy");
            let reopened = HistoryStore::new(dir.path().to_path_buf());
            assert_eq!(reopened.load(&conv.id).expect("load").title, "Legacy");
            assert_eq!(fs::read(&path).expect("unchanged"), legacy);
            reopened
                .rename(&conv.id, "Migrated".into())
                .expect("migrate");
            let file: ConversationsFile =
                serde_json::from_slice(&fs::read(&path).expect("read")).expect("parse");
            assert_eq!(file.version, CURRENT_SCHEMA_VERSION);
        }
    }

    #[test]
    fn unsupported_version_blocks_every_mutation() {
        let dir = TempDir::new().expect("dir");
        let path = dir.path().join(CONVERSATIONS_FILE);
        let data = br#"{"version":999,"conversations":[]}"#;
        fs::write(&path, data).expect("future");
        let store = HistoryStore::new(dir.path().to_path_buf());
        assert_eq!(store.list().unwrap_err().code, "unsupported_version");
        assert!(store.create(None).is_err());
        assert!(store.delete("anything").is_err());
        assert_eq!(fs::read(&path).expect("preserved"), data);
    }
}
