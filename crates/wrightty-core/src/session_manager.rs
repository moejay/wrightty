use std::collections::HashMap;

use wrightty_protocol::types::{SessionId, SessionInfo};

use crate::session::{Session, SessionError};

pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
    max_sessions: usize,
}

impl SessionManager {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            max_sessions,
        }
    }

    pub fn create(
        &mut self,
        shell: Option<String>,
        args: Vec<String>,
        cols: u16,
        rows: u16,
        env: std::collections::HashMap<String, String>,
        cwd: Option<String>,
    ) -> Result<SessionId, SessionError> {
        if self.sessions.len() >= self.max_sessions {
            return Err(SessionError::Spawn("max sessions reached".to_string()));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let session = Session::spawn(id.clone(), shell, args, cols, rows, env, cwd)?;
        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    pub fn destroy(&mut self, id: &str) -> Result<(), SessionError> {
        self.sessions
            .remove(id)
            .ok_or_else(|| SessionError::Spawn(format!("session not found: {id}")))?;
        // Session::drop will kill the child and abort the reader
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions
            .values()
            .map(|s| {
                let (cols, rows) = s.size();
                SessionInfo {
                    session_id: s.id.clone(),
                    title: s.title.clone(),
                    cwd: None,
                    cols,
                    rows,
                    pid: None,
                    running: s.is_running(),
                    alternate_screen: false,
                }
            })
            .collect()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
