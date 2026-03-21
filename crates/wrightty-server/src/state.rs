use std::sync::{Arc, Mutex};

use wrightty_core::session_manager::SessionManager;

#[derive(Clone)]
pub struct AppState {
    pub session_manager: Arc<Mutex<SessionManager>>,
}

impl AppState {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            session_manager: Arc::new(Mutex::new(SessionManager::new(max_sessions))),
        }
    }
}
