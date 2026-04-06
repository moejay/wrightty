use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use wrightty_core::session_manager::SessionManager;

/// A single frame captured during a video recording.
#[derive(Clone)]
pub struct VideoFrame {
    pub elapsed_secs: f64,
    pub text: String,
}

/// State for an active video recording.
pub struct VideoRecording {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: Instant,
    pub interval_ms: u64,
    pub frames: Vec<VideoFrame>,
    pub running: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub session_manager: Arc<Mutex<SessionManager>>,
    /// Active video recordings keyed by recording ID.
    pub video_recordings: Arc<Mutex<HashMap<String, VideoRecording>>>,
    /// Optional human-readable server name.
    pub name: Option<String>,
    /// Optional password for client authentication.
    pub password: Option<String>,
    /// Set of connection IDs that have successfully authenticated.
    pub authenticated_connections: Arc<Mutex<HashSet<usize>>>,
}

impl AppState {
    pub fn new(max_sessions: usize, name: Option<String>, password: Option<String>) -> Self {
        Self {
            session_manager: Arc::new(Mutex::new(SessionManager::new(max_sessions))),
            video_recordings: Arc::new(Mutex::new(HashMap::new())),
            name,
            password,
            authenticated_connections: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}
