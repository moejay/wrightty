use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tokio::sync::broadcast;
use vte::ansi::Processor;

use wrightty_protocol::types::SessionId;

use crate::screen;

/// Terminal dimensions for alacritty_terminal.
struct TermSize {
    cols: usize,
    lines: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Internal terminal state protected by a mutex.
pub struct TermState {
    pub term: Term<VoidListener>,
    pub parser: Processor,
}

/// A terminal session: owns a PTY, a virtual terminal, and the reader task.
pub struct Session {
    pub id: SessionId,
    pub state: Arc<Mutex<TermState>>,
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub update_tx: broadcast::Sender<()>,
    reader_handle: tokio::task::JoinHandle<()>,
    cols: u16,
    rows: u16,
    pub title: String,
}

impl Session {
    pub fn spawn(
        id: SessionId,
        shell: Option<String>,
        args: Vec<String>,
        cols: u16,
        rows: u16,
        env: std::collections::HashMap<String, String>,
        cwd: Option<String>,
    ) -> Result<Self, SessionError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SessionError::Spawn(e.to_string()))?;

        let shell_path = shell.unwrap_or_else(|| {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        });

        let mut cmd = CommandBuilder::new(&shell_path);
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.env("TERM", "xterm-256color");
        for (k, v) in &env {
            cmd.env(k, v);
        }
        if let Some(ref dir) = cwd {
            cmd.cwd(dir);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| SessionError::Spawn(e.to_string()))?;

        // Drop the slave side — we only talk through the master
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| SessionError::Spawn(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| SessionError::Spawn(e.to_string()))?;

        // Create the virtual terminal
        let size = TermSize {
            cols: cols as usize,
            lines: rows as usize,
        };
        let term = Term::new(TermConfig::default(), &size, VoidListener);
        let parser = Processor::new();

        let state = Arc::new(Mutex::new(TermState { term, parser }));
        let (update_tx, _) = broadcast::channel(64);

        // Spawn a blocking reader task that feeds PTY output into the terminal
        let reader_state = Arc::clone(&state);
        let reader_tx = update_tx.clone();
        let reader_handle = tokio::task::spawn_blocking(move || {
            Self::reader_loop(reader, reader_state, reader_tx);
        });

        Ok(Session {
            id,
            state,
            master: pair.master,
            writer,
            child,
            update_tx,
            reader_handle,
            cols,
            rows,
            title: shell_path,
        })
    }

    fn reader_loop(
        mut reader: Box<dyn Read + Send>,
        state: Arc<Mutex<TermState>>,
        update_tx: broadcast::Sender<()>,
    ) {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF — child exited
                Ok(n) => {
                    let mut s = state.lock().unwrap();
                    let TermState { ref mut parser, ref mut term } = *s;
                    parser.advance(term, &buf[..n]);
                    drop(s);
                    // Notify subscribers (ignore error if no receivers)
                    let _ = update_tx.send(());
                }
                Err(_) => break,
            }
        }
    }

    /// Send raw bytes to the PTY.
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), SessionError> {
        self.writer
            .write_all(data)
            .map_err(|e| SessionError::Io(e.to_string()))?;
        self.writer
            .flush()
            .map_err(|e| SessionError::Io(e.to_string()))?;
        Ok(())
    }

    /// Resize the PTY and terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), SessionError> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SessionError::Io(e.to_string()))?;

        let size = TermSize {
            cols: cols as usize,
            lines: rows as usize,
        };
        let mut s = self.state.lock().unwrap();
        s.term.resize(size);
        drop(s);

        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Get the current screen as plain text.
    pub fn get_text(&self) -> String {
        let s = self.state.lock().unwrap();
        screen::extract_text(&s.term)
    }

    /// Get the current screen as a structured cell grid.
    pub fn get_contents(&self) -> screen::ScreenGetContentsData {
        let s = self.state.lock().unwrap();
        screen::extract_contents(&s.term)
    }

    /// Get scrollback history lines.
    pub fn get_scrollback(
        &self,
        lines: u32,
        offset: u32,
    ) -> (Vec<wrightty_protocol::methods::ScrollbackLine>, u32) {
        let s = self.state.lock().unwrap();
        screen::extract_scrollback(&s.term, lines, offset)
    }

    /// Get the current terminal size.
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Check if the child process is still running.
    pub fn is_running(&self) -> bool {
        // try_wait returns Ok(Some(status)) if exited, Ok(None) if still running
        // portable-pty Child doesn't have try_wait in the trait, so we'll just
        // return true for now and handle exit via the reader loop
        true
    }

    /// Get a broadcast receiver for screen update notifications.
    pub fn subscribe_updates(&self) -> broadcast::Receiver<()> {
        self.update_tx.subscribe()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.reader_handle.abort();
        let _ = self.child.kill();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("failed to spawn session: {0}")]
    Spawn(String),
    #[error("I/O error: {0}")]
    Io(String),
}
