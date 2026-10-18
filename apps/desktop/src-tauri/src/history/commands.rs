use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use super::storage::{
    ConversationSummary, HistoryError, HistoryResult, HistoryStore, StoredConversation,
};

// Tauri state wrapper so lib.rs can manage it separately from InferenceState
pub struct HistoryState {
    pub store: Mutex<HistoryStore>,
}

impl HistoryState {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            store: Mutex::new(HistoryStore::new(config_dir)),
        }
    }
}

fn lock_err() -> HistoryError {
    HistoryError::storage("Conversation storage is unavailable. Restart Nexora.")
}

/// List all conversations as summaries, most recent first.
#[tauri::command]
pub fn list_conversations(state: State<HistoryState>) -> HistoryResult<Vec<ConversationSummary>> {
    let store = state.store.lock().map_err(|_| lock_err())?;
    store.list()
}

/// Load a single conversation by id.
#[tauri::command]
pub fn load_conversation(
    state: State<HistoryState>,
    id: String,
) -> HistoryResult<StoredConversation> {
    let store = state.store.lock().map_err(|_| lock_err())?;
    store.load(&id)
}

/// Create a new empty conversation. If `id` is provided it is validated, otherwise server generates one.
#[tauri::command]
pub fn create_conversation(
    state: State<HistoryState>,
    id: Option<String>,
    title: Option<String>,
) -> HistoryResult<StoredConversation> {
    let store = state.store.lock().map_err(|_| lock_err())?;
    if let Some(provided) = id {
        store.create_with_id(provided, title)
    } else {
        store.create(title)
    }
}

/// Save or update a conversation and its messages. Creates if id is new.
#[tauri::command]
pub fn save_conversation(
    state: State<HistoryState>,
    conversation: StoredConversation,
) -> HistoryResult<StoredConversation> {
    let store = state.store.lock().map_err(|_| lock_err())?;
    store.save(conversation)
}

/// Rename a conversation.
#[tauri::command]
pub fn rename_conversation(
    state: State<HistoryState>,
    id: String,
    title: String,
) -> HistoryResult<ConversationSummary> {
    let store = state.store.lock().map_err(|_| lock_err())?;
    store.rename(&id, title)
}

/// Delete a conversation by id. Returns true if deleted.
#[tauri::command]
pub fn delete_conversation(state: State<HistoryState>, id: String) -> HistoryResult<bool> {
    let store = state.store.lock().map_err(|_| lock_err())?;
    store.delete(&id)
}
