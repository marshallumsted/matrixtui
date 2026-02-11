use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use matrix_sdk::ruma::OwnedRoomId;
use ratatui::prelude::*;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::account::{Account, MatrixEvent, RoomInfo};
use crate::config::{Config, SavedAccount};
use crate::event::{AppEvent, spawn_input_reader, spawn_matrix_bridge};
use crate::ui;

/// Which panel has focus
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Accounts,
    Rooms,
    Chat,
    Input,
    LoginOverlay,
}

/// Which overlay is showing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overlay {
    None,
    Login,
    Help,
    RoomSwitcher,
}

/// A message stored for display
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub sender: String,
    pub body: String,
    pub timestamp: u64,
}

pub struct App {
    pub config: Config,
    pub accounts: Vec<Account>,
    pub focus: Focus,
    pub overlay: Overlay,
    pub running: bool,

    // Room state
    pub all_rooms: Vec<RoomInfo>,
    pub selected_room: usize,
    pub active_room: Option<OwnedRoomId>,
    pub active_account_id: Option<String>,

    // Chat state
    pub messages: Vec<DisplayMessage>,
    pub scroll_offset: usize,

    // Input state
    pub input: String,
    pub cursor_pos: usize,

    // Login form state
    pub login_homeserver: String,
    pub login_username: String,
    pub login_password: String,
    pub login_focus: usize, // 0=homeserver, 1=username, 2=password
    pub login_error: Option<String>,
    pub login_busy: bool,

    // Room switcher state
    pub switcher_query: String,
    pub switcher_selected: usize,

    // Status
    pub status_msg: String,

    // Selected account in account list
    pub selected_account: usize,

    // Channels
    matrix_tx: mpsc::UnboundedSender<MatrixEvent>,
    matrix_rx: Option<mpsc::UnboundedReceiver<MatrixEvent>>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let (matrix_tx, matrix_rx) = mpsc::unbounded_channel();
        Self {
            config,
            accounts: Vec::new(),
            focus: Focus::Rooms,
            overlay: Overlay::None,
            running: true,
            all_rooms: Vec::new(),
            selected_room: 0,
            active_room: None,
            active_account_id: None,
            messages: Vec::new(),
            scroll_offset: 0,
            input: String::new(),
            cursor_pos: 0,
            login_homeserver: String::new(),
            login_username: String::new(),
            login_password: String::new(),
            login_focus: 0,
            login_error: None,
            login_busy: false,
            switcher_query: String::new(),
            switcher_selected: 0,
            status_msg: "No accounts — press 'a' to add one".to_string(),
            selected_account: 0,
            matrix_tx,
            matrix_rx: Some(matrix_rx),
        }
    }

    /// Restore all saved sessions on startup
    pub async fn restore_sessions(&mut self) {
        let saved = self.config.accounts.clone();
        for sa in &saved {
            match Account::restore(sa).await {
                Ok(mut account) => {
                    info!("Restored session for {}", account.user_id);
                    account.start_sync(self.matrix_tx.clone());
                    self.accounts.push(account);
                }
                Err(e) => {
                    error!("Failed to restore {}: {}", sa.user_id, e);
                }
            }
        }
        self.refresh_rooms();
        if !self.accounts.is_empty() {
            self.status_msg = format!("{} account(s) connected", self.accounts.len());
        }
    }

    /// Main event loop
    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        let (app_tx, mut app_rx) = mpsc::unbounded_channel();

        // Start input reader
        spawn_input_reader(app_tx.clone());

        // Bridge matrix events to app events
        if let Some(mrx) = self.matrix_rx.take() {
            spawn_matrix_bridge(mrx, app_tx.clone());
        }

        while self.running {
            terminal.draw(|f| ui::draw(f, self))?;

            if let Some(event) = app_rx.recv().await {
                match event {
                    AppEvent::Key(key) => self.handle_key(key).await,
                    AppEvent::Resize(_, _) => {} // ratatui handles this on next draw
                    AppEvent::Matrix(mev) => self.handle_matrix_event(mev),
                    AppEvent::Tick => {}
                }
            }
        }

        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) {
        // Global shortcuts first
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                self.running = false;
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                self.overlay = Overlay::RoomSwitcher;
                self.switcher_query.clear();
                self.switcher_selected = 0;
                return;
            }
            _ => {}
        }

        // Route to overlay or focused panel
        match self.overlay {
            Overlay::Login => self.handle_login_key(key).await,
            Overlay::Help => {
                if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::RoomSwitcher => self.handle_switcher_key(key),
            Overlay::None => match self.focus {
                Focus::Accounts => self.handle_accounts_key(key),
                Focus::Rooms => self.handle_rooms_key(key),
                Focus::Chat => self.handle_chat_key(key),
                Focus::Input => self.handle_input_key(key).await,
                Focus::LoginOverlay => {}
            },
        }
    }

    fn handle_accounts_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('a') => {
                self.overlay = Overlay::Login;
                self.login_homeserver = "matrix.org".to_string();
                self.login_username.clear();
                self.login_password.clear();
                self.login_focus = 0;
                self.login_error = None;
            }
            KeyCode::Up => {
                if self.selected_account > 0 {
                    self.selected_account -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_account + 1 < self.accounts.len() {
                    self.selected_account += 1;
                }
            }
            KeyCode::Tab => self.focus = Focus::Rooms,
            KeyCode::Right => self.focus = Focus::Rooms,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            _ => {}
        }
    }

    fn handle_rooms_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.selected_room > 0 {
                    self.selected_room -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_room + 1 < self.all_rooms.len() {
                    self.selected_room += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(room) = self.all_rooms.get(self.selected_room) {
                    self.active_room = Some(room.id.clone());
                    self.active_account_id = Some(room.account_id.clone());
                    self.messages.clear();
                    self.scroll_offset = 0;
                    self.focus = Focus::Chat;
                    self.status_msg = format!("Opened {}", room.name);
                }
            }
            KeyCode::Tab => self.focus = Focus::Chat,
            KeyCode::BackTab => self.focus = Focus::Accounts,
            KeyCode::Left => self.focus = Focus::Accounts,
            KeyCode::Right => self.focus = Focus::Chat,
            KeyCode::Char('a') => {
                self.overlay = Overlay::Login;
                self.login_homeserver = "matrix.org".to_string();
                self.login_username.clear();
                self.login_password.clear();
                self.login_focus = 0;
                self.login_error = None;
            }
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            _ => {}
        }
    }

    fn handle_chat_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Enter => {
                self.focus = Focus::Input;
            }
            KeyCode::Tab => self.focus = Focus::Input,
            KeyCode::BackTab => self.focus = Focus::Rooms,
            KeyCode::Left => self.focus = Focus::Rooms,
            KeyCode::Esc => self.focus = Focus::Rooms,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            _ => {}
        }
    }

    async fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let msg = self.input.clone();
                    self.input.clear();
                    self.cursor_pos = 0;
                    self.send_current_message(&msg).await;
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.input.remove(self.cursor_pos);
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.input.len() {
                    self.input.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.cursor_pos < self.input.len() {
                    self.cursor_pos += 1;
                }
            }
            KeyCode::Home => self.cursor_pos = 0,
            KeyCode::End => self.cursor_pos = self.input.len(),
            KeyCode::Esc => {
                self.focus = Focus::Chat;
            }
            KeyCode::Tab => self.focus = Focus::Rooms,
            _ => {}
        }
    }

    async fn handle_login_key(&mut self, key: KeyEvent) {
        if self.login_busy {
            return;
        }

        match key.code {
            KeyCode::Tab => {
                self.login_focus = (self.login_focus + 1) % 3;
            }
            KeyCode::BackTab => {
                self.login_focus = if self.login_focus == 0 { 2 } else { self.login_focus - 1 };
            }
            KeyCode::Enter => {
                if self.login_focus == 2 || (!self.login_username.is_empty() && !self.login_password.is_empty()) {
                    self.do_login().await;
                } else {
                    self.login_focus = (self.login_focus + 1) % 3;
                }
            }
            KeyCode::Esc => {
                self.overlay = Overlay::None;
            }
            KeyCode::Char(c) => {
                let field = match self.login_focus {
                    0 => &mut self.login_homeserver,
                    1 => &mut self.login_username,
                    2 => &mut self.login_password,
                    _ => return,
                };
                field.push(c);
            }
            KeyCode::Backspace => {
                let field = match self.login_focus {
                    0 => &mut self.login_homeserver,
                    1 => &mut self.login_username,
                    2 => &mut self.login_password,
                    _ => return,
                };
                field.pop();
            }
            _ => {}
        }
    }

    fn handle_switcher_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
            }
            KeyCode::Enter => {
                let filtered = self.filtered_rooms();
                if let Some(room) = filtered.get(self.switcher_selected) {
                    self.active_room = Some(room.id.clone());
                    self.active_account_id = Some(room.account_id.clone());
                    self.messages.clear();
                    self.scroll_offset = 0;
                    self.focus = Focus::Chat;
                    self.overlay = Overlay::None;
                    // Update selected_room to match
                    if let Some(idx) = self.all_rooms.iter().position(|r| r.id == room.id) {
                        self.selected_room = idx;
                    }
                }
            }
            KeyCode::Up => {
                self.switcher_selected = self.switcher_selected.saturating_sub(1);
            }
            KeyCode::Down => {
                let count = self.filtered_rooms().len();
                if self.switcher_selected + 1 < count {
                    self.switcher_selected += 1;
                }
            }
            KeyCode::Char(c) => {
                self.switcher_query.push(c);
                self.switcher_selected = 0;
            }
            KeyCode::Backspace => {
                self.switcher_query.pop();
                self.switcher_selected = 0;
            }
            _ => {}
        }
    }

    async fn do_login(&mut self) {
        self.login_busy = true;
        self.login_error = None;
        self.status_msg = format!("Logging in to {}...", self.login_homeserver);

        match Account::login(&self.login_homeserver, &self.login_username, &self.login_password).await {
            Ok((mut account, saved)) => {
                info!("Logged in as {}", account.user_id);
                account.start_sync(self.matrix_tx.clone());
                self.config.add_account(saved);
                if let Err(e) = self.config.save() {
                    error!("Failed to save config: {}", e);
                }
                self.status_msg = format!("Logged in as {}", account.user_id);
                self.accounts.push(account);
                self.refresh_rooms();
                self.overlay = Overlay::None;
            }
            Err(e) => {
                error!("Login failed: {}", e);
                self.login_error = Some(e.to_string());
                self.status_msg = "Login failed".to_string();
            }
        }
        self.login_busy = false;
    }

    async fn send_current_message(&mut self, body: &str) {
        let room_id = match &self.active_room {
            Some(id) => id.clone(),
            None => return,
        };
        let account_id = match &self.active_account_id {
            Some(id) => id.clone(),
            None => return,
        };

        if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
            if let Err(e) = account.send_message(&room_id, body).await {
                self.status_msg = format!("Send failed: {}", e);
            }
        }
    }

    fn handle_matrix_event(&mut self, event: MatrixEvent) {
        match event {
            MatrixEvent::Message {
                room_id,
                sender,
                body,
                timestamp,
            } => {
                // If this message is for the active room, add to display
                if Some(&room_id) == self.active_room.as_ref() {
                    self.messages.push(DisplayMessage {
                        sender: sender.to_string(),
                        body,
                        timestamp,
                    });
                }
            }
            MatrixEvent::RoomsUpdated { .. } => {
                self.refresh_rooms();
            }
            MatrixEvent::SyncComplete { account_id } => {
                self.status_msg = format!("{}: synced", account_id);
                self.refresh_rooms();
            }
            MatrixEvent::SyncError { account_id, error } => {
                self.status_msg = format!("{}: sync error — {}", account_id, error);
            }
        }
    }

    pub fn refresh_rooms(&mut self) {
        self.all_rooms.clear();
        for account in &self.accounts {
            self.all_rooms.extend(account.rooms());
        }
        // Sort: rooms with unread first, then alphabetical
        self.all_rooms.sort_by(|a, b| {
            b.unread
                .cmp(&a.unread)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }

    pub fn filtered_rooms(&self) -> Vec<RoomInfo> {
        if self.switcher_query.is_empty() {
            return self.all_rooms.clone();
        }
        let q = self.switcher_query.to_lowercase();
        self.all_rooms
            .iter()
            .filter(|r| r.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
}
