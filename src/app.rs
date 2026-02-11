use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use matrix_sdk::ruma::OwnedRoomId;
use ratatui::prelude::*;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::account::{Account, MatrixEvent, RoomInfo};
use crate::config::Config;
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
    Settings,
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
    pub room_messages: HashMap<OwnedRoomId, Vec<DisplayMessage>>,
    pending_echoes: Vec<String>,

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

    // Settings overlay state
    pub settings_selected: usize,          // 0=Accounts, 1=Theme
    pub settings_accounts_open: bool,
    pub settings_accounts_selected: usize, // 0=Add Account, 1..=N for accounts
    pub settings_account_action_open: bool,
    pub settings_account_action_selected: usize, // 0=Reconnect, 1=Remove
    pub settings_theme_open: bool,
    pub settings_theme_selected: usize,

    // Active theme
    pub theme: ui::Theme,

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
        let theme = ui::theme_by_name(&config.theme);
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
            room_messages: HashMap::new(),
            pending_echoes: Vec::new(),
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
            settings_selected: 0,
            settings_accounts_open: false,
            settings_accounts_selected: 0,
            settings_account_action_open: false,
            settings_account_action_selected: 0,
            settings_theme_open: false,
            settings_theme_selected: 0,
            theme,
            status_msg: "No accounts — press 'a' to add one".to_string(),
            selected_account: 0,
            matrix_tx,
            matrix_rx: Some(matrix_rx),
        }
    }

    /// Restore all saved sessions on startup
    pub async fn restore_sessions(&mut self) {
        let saved = self.config.accounts.clone();
        let mut errors = Vec::new();
        for sa in &saved {
            self.status_msg = format!("Restoring {}...", sa.user_id);
            match Account::restore(sa).await {
                Ok(mut account) => {
                    info!("Restored session for {}", account.user_id);
                    account.start_sync(self.matrix_tx.clone());
                    self.accounts.push(account);
                }
                Err(e) => {
                    error!("Failed to restore {}: {}", sa.user_id, e);
                    errors.push(format!("{}: {}", sa.user_id, e));
                }
            }
        }
        self.refresh_rooms().await;
        if !errors.is_empty() {
            self.status_msg = format!("Restore failed: {}", errors.join("; "));
        } else if !self.accounts.is_empty() {
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
                    AppEvent::Matrix(mev) => self.handle_matrix_event(mev).await,
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
            Overlay::RoomSwitcher => self.handle_switcher_key(key).await,
            Overlay::Settings => self.handle_settings_key(key).await,
            Overlay::None => match self.focus {
                Focus::Accounts => self.handle_accounts_key(key),
                Focus::Rooms => self.handle_rooms_key(key).await,
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
            KeyCode::Char('s') => {
                self.overlay = Overlay::Settings;
                self.settings_selected = 0;
                self.settings_accounts_open = false;
                self.settings_account_action_open = false;
                self.settings_theme_open = false;
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

    async fn handle_rooms_key(&mut self, key: KeyEvent) {
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
                self.open_selected_room().await;
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
            KeyCode::Char('s') => {
                self.overlay = Overlay::Settings;
                self.settings_selected = 0;
                self.settings_accounts_open = false;
                self.settings_account_action_open = false;
                self.settings_theme_open = false;
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

    async fn handle_switcher_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
            }
            KeyCode::Enter => {
                let filtered = self.filtered_rooms();
                if let Some(room) = filtered.get(self.switcher_selected) {
                    // Update selected_room to match
                    if let Some(idx) = self.all_rooms.iter().position(|r| r.id == room.id) {
                        self.selected_room = idx;
                    }
                    self.overlay = Overlay::None;
                    self.open_selected_room().await;
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

    async fn handle_settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.settings_account_action_open {
                    self.settings_account_action_open = false;
                } else if self.settings_accounts_open {
                    self.settings_accounts_open = false;
                } else if self.settings_theme_open {
                    self.settings_theme_open = false;
                } else {
                    self.overlay = Overlay::None;
                }
            }
            KeyCode::Up => {
                if self.settings_account_action_open {
                    self.settings_account_action_selected =
                        self.settings_account_action_selected.saturating_sub(1);
                } else if self.settings_accounts_open {
                    self.settings_accounts_selected =
                        self.settings_accounts_selected.saturating_sub(1);
                } else if self.settings_theme_open {
                    self.settings_theme_selected =
                        self.settings_theme_selected.saturating_sub(1);
                } else {
                    self.settings_selected = self.settings_selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if self.settings_account_action_open {
                    if self.settings_account_action_selected < 1 {
                        self.settings_account_action_selected += 1;
                    }
                } else if self.settings_accounts_open {
                    let count = 1 + self.accounts.len(); // Add Account + each account
                    if self.settings_accounts_selected + 1 < count {
                        self.settings_accounts_selected += 1;
                    }
                } else if self.settings_theme_open {
                    let count = ui::builtin_themes().len();
                    if self.settings_theme_selected + 1 < count {
                        self.settings_theme_selected += 1;
                    }
                } else if self.settings_selected < 1 {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Enter => {
                if self.settings_account_action_open {
                    let acct_idx = self.settings_accounts_selected - 1;
                    match self.settings_account_action_selected {
                        0 => {
                            // Reconnect
                            self.reconnect_account(acct_idx).await;
                            self.settings_account_action_open = false;
                        }
                        1 => {
                            // Remove
                            self.remove_account_by_index(acct_idx).await;
                            self.settings_account_action_open = false;
                            // Clamp selection
                            let count = 1 + self.accounts.len();
                            if self.settings_accounts_selected >= count {
                                self.settings_accounts_selected = count.saturating_sub(1);
                            }
                        }
                        _ => {}
                    }
                } else if self.settings_accounts_open {
                    if self.settings_accounts_selected == 0 {
                        // Add Account
                        self.overlay = Overlay::Login;
                        self.login_homeserver = "matrix.org".to_string();
                        self.login_username.clear();
                        self.login_password.clear();
                        self.login_focus = 0;
                        self.login_error = None;
                    } else {
                        // Open action menu for selected account
                        self.settings_account_action_open = true;
                        self.settings_account_action_selected = 0;
                    }
                } else if self.settings_theme_open {
                    let themes = ui::builtin_themes();
                    if let Some(t) = themes.get(self.settings_theme_selected) {
                        self.theme = t.clone();
                        self.config.theme = t.name.to_string();
                        let _ = self.config.save();
                    }
                    self.settings_theme_open = false;
                } else if self.settings_selected == 0 {
                    // Open accounts sub-menu
                    self.settings_accounts_open = true;
                    self.settings_theme_open = false;
                    self.settings_accounts_selected = 0;
                } else if self.settings_selected == 1 {
                    // Open theme picker
                    self.settings_theme_open = true;
                    self.settings_accounts_open = false;
                    let themes = ui::builtin_themes();
                    self.settings_theme_selected = themes
                        .iter()
                        .position(|t| t.name == self.theme.name)
                        .unwrap_or(0);
                }
            }
            _ => {}
        }
    }

    async fn reconnect_account(&mut self, idx: usize) {
        if idx >= self.accounts.len() {
            return;
        }
        let user_id = self.accounts[idx].user_id.clone();
        self.status_msg = format!("Reconnecting {}...", user_id);

        // Remove the old account (drops client, stops sync)
        self.accounts.remove(idx);

        // Re-restore from saved config
        if let Some(saved) = self.config.accounts.iter().find(|a| a.user_id == user_id) {
            let saved = saved.clone();
            match Account::restore(&saved).await {
                Ok(mut account) => {
                    account.start_sync(self.matrix_tx.clone());
                    self.status_msg = format!("Reconnected {}", account.user_id);
                    self.accounts.push(account);
                }
                Err(e) => {
                    self.status_msg = format!("Reconnect failed: {}", user_id);
                    error!("Reconnect failed for {}: {}", user_id, e);
                }
            }
        }
        self.refresh_rooms().await;
    }

    async fn remove_account_by_index(&mut self, idx: usize) {
        if idx >= self.accounts.len() {
            return;
        }
        let user_id = self.accounts[idx].user_id.clone();

        // Remove from active accounts (drops client, stops sync)
        self.accounts.remove(idx);

        // Remove from config
        self.config.remove_account(&user_id);
        if let Err(e) = self.config.save() {
            error!("Failed to save config: {}", e);
        }

        // Clear active room if it belonged to this account
        if self.active_account_id.as_deref() == Some(&user_id) {
            self.active_room = None;
            self.active_account_id = None;
            self.messages.clear();
        }

        self.status_msg = format!("Removed {}", user_id);
        self.refresh_rooms().await;

        if self.accounts.is_empty() {
            self.status_msg = "No accounts \u{2014} press 's' to add one".to_string();
        }
    }

    async fn do_login(&mut self) {
        self.login_busy = true;
        self.login_error = None;

        // Check if already logged in to this homeserver with this username
        let check_id = format!("@{}:{}", self.login_username, self.login_homeserver);
        if self.accounts.iter().any(|a| a.user_id == check_id || a.user_id == self.login_username) {
            self.login_error = Some("Already logged in to this account".to_string());
            self.login_busy = false;
            return;
        }

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
                self.refresh_rooms().await;
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
            match account.send_message(&room_id, body).await {
                Ok(_) => {
                    // Local echo — show our own message immediately
                    let msg = DisplayMessage {
                        sender: account.user_id.clone(),
                        body: body.to_string(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };
                    self.messages.push(msg.clone());
                    self.room_messages
                        .entry(room_id)
                        .or_default()
                        .push(msg);
                    self.pending_echoes.push(body.to_string());
                    self.scroll_offset = 0;
                }
                Err(e) => {
                    self.status_msg = format!("Send failed: {}", e);
                }
            }
        }
    }

    async fn handle_matrix_event(&mut self, event: MatrixEvent) {
        match event {
            MatrixEvent::Message {
                room_id,
                sender,
                body,
                timestamp,
            } => {
                // Skip if this is our own message echoed back from sync
                if let Some(pos) = self.pending_echoes.iter().position(|b| *b == body) {
                    let is_own = self.accounts.iter().any(|a| a.user_id == sender.as_str());
                    if is_own {
                        self.pending_echoes.remove(pos);
                        return;
                    }
                }

                let msg = DisplayMessage {
                    sender: sender.to_string(),
                    body,
                    timestamp,
                };

                // Always cache in per-room store
                self.room_messages
                    .entry(room_id.clone())
                    .or_default()
                    .push(msg.clone());

                // If this message is for the active room, add to display
                if Some(&room_id) == self.active_room.as_ref() {
                    self.messages.push(msg);
                }
            }
            MatrixEvent::RoomsUpdated { .. } => {
                self.refresh_rooms().await;
            }
            MatrixEvent::SyncComplete { account_id } => {
                info!("SyncComplete for {}", account_id);
                if let Some(acct) = self.accounts.iter_mut().find(|a| a.user_id == account_id) {
                    acct.sync_complete = true;
                }

                // Update status to reflect actual per-account sync state
                let states: Vec<_> = self.accounts.iter()
                    .map(|a| {
                        let state = if a.sync_complete { "synced" } else { "syncing" };
                        format!("{}: {}", a.homeserver, state)
                    })
                    .collect();
                self.status_msg = states.join(" | ");
                self.refresh_rooms().await;

                // Re-fetch history if viewing a room from this account with empty messages
                if let (Some(room_id), Some(active_aid)) =
                    (self.active_room.clone(), self.active_account_id.clone())
                {
                    if active_aid == account_id && self.messages.is_empty() {
                        info!("Re-fetching history after sync for {}", account_id);
                        if let Some(account) =
                            self.accounts.iter().find(|a| a.user_id == account_id)
                        {
                            match account.fetch_history(&room_id, 50).await {
                                Ok(msgs) => {
                                    let count = msgs.len();
                                    self.messages = msgs;
                                    info!("Re-fetch got {} messages", count);
                                }
                                Err(e) => {
                                    info!("Re-fetch failed: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            MatrixEvent::SyncError { account_id, error } => {
                if let Some(acct) = self.accounts.iter_mut().find(|a| a.user_id == account_id) {
                    acct.syncing = false;
                    acct.sync_complete = false;
                }
                self.status_msg = format!("{}: sync error — {}", account_id, error);
            }
        }
    }

    pub async fn refresh_rooms(&mut self) {
        self.all_rooms.clear();
        for account in &self.accounts {
            self.all_rooms.extend(account.rooms().await);
        }
        // Sort: rooms with unread first, then alphabetical
        self.all_rooms.sort_by(|a, b| {
            b.unread
                .cmp(&a.unread)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }

    async fn open_selected_room(&mut self) {
        if let Some(room) = self.all_rooms.get(self.selected_room) {
            let room_id = room.id.clone();
            let account_id = room.account_id.clone();
            let room_name = room.name.clone();

            // Save current room's messages before switching
            if let Some(prev_room_id) = &self.active_room {
                if !self.messages.is_empty() {
                    self.room_messages
                        .insert(prev_room_id.clone(), self.messages.clone());
                }
            }

            self.active_room = Some(room_id.clone());
            self.active_account_id = Some(account_id.clone());
            self.messages.clear();
            self.scroll_offset = 0;
            self.focus = Focus::Chat;

            let account_synced = self
                .accounts
                .iter()
                .find(|a| a.user_id == account_id)
                .map(|a| a.sync_complete)
                .unwrap_or(false);

            if !account_synced {
                self.status_msg = format!("{} — waiting for sync...", room_name);
            } else {
                self.status_msg = format!("Loading {}...", room_name);
            }

            // Try fetch_history first
            if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
                match account.fetch_history(&room_id, 50).await {
                    Ok(msgs) if !msgs.is_empty() => {
                        let count = msgs.len();
                        self.messages = msgs;
                        self.status_msg = format!(
                            "{} ({}) — {} messages",
                            room_name, account_id, count
                        );
                    }
                    Ok(_) => {
                        // fetch_history returned empty — fall back to cached messages from sync
                        if let Some(cached) = self.room_messages.get(&room_id) {
                            let count = cached.len();
                            self.messages = cached.clone();
                            self.status_msg = format!(
                                "{} ({}) — {} cached messages",
                                room_name, account_id, count
                            );
                        } else if account_synced {
                            self.status_msg =
                                format!("{} ({}) — no messages", room_name, account_id);
                        }
                        // If not synced, status already says "waiting for sync"
                    }
                    Err(e) => {
                        // History fetch failed — try cache
                        info!("fetch_history error for {}: {}", room_id, e);
                        if let Some(cached) = self.room_messages.get(&room_id) {
                            let count = cached.len();
                            self.messages = cached.clone();
                            self.status_msg = format!(
                                "{} ({}) — {} cached messages (history error)",
                                room_name, account_id, count
                            );
                        } else {
                            self.status_msg =
                                format!("{} ({}) — history failed: {}", room_name, account_id, e);
                        }
                    }
                }
            } else {
                self.status_msg = format!(
                    "{} — account not found: {} (have: {})",
                    room_name,
                    account_id,
                    self.accounts
                        .iter()
                        .map(|a| a.user_id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
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
