use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use matrix_sdk::ruma::OwnedRoomId;
use ratatui::prelude::*;
use std::cell::Cell;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::account::{Account, MatrixEvent, RoomInfo};
use crate::config::Config;
use crate::event::{AppEvent, spawn_input_reader, spawn_matrix_bridge};
use crate::ui;

/// How rooms (outside favorites) are sorted
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoomSortMode {
    Unread,
    Recent,
    Alpha,
}

impl RoomSortMode {
    pub const ALL: [RoomSortMode; 3] = [
        RoomSortMode::Unread,
        RoomSortMode::Recent,
        RoomSortMode::Alpha,
    ];

    pub fn from_str(s: &str) -> Self {
        match s {
            "recent" => Self::Recent,
            "alpha" => Self::Alpha,
            _ => Self::Unread,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Recent => "recent",
            Self::Alpha => "alpha",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Unread => "Unread First",
            Self::Recent => "Recent Activity",
            Self::Alpha => "Alphabetical",
        }
    }
}

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
    ProfileEditor,
    RoomCreator,
    RoomEditor,
    Recovery,
    MessageAction,
}

/// A message stored for display
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub event_id: Option<String>,
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
    pub downloading_keys: bool,

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

    // Sort & favorites
    pub room_sort: RoomSortMode,
    pub favorites_count: usize,
    pub settings_sort_open: bool,
    pub settings_sort_selected: usize,

    // Profile editor overlay state
    pub profile_display_name: String,
    pub profile_avatar_url: String,
    pub profile_avatar_path: String,
    pub profile_focus: usize,
    pub profile_error: Option<String>,
    pub profile_busy: bool,
    pub profile_account_idx: usize,
    pub profile_current_name: String,
    pub profile_current_avatar: String,

    // Room creator overlay state
    pub creator_name: String,
    pub creator_topic: String,
    pub creator_visibility: usize,
    pub creator_e2ee: bool,
    pub creator_federated: bool,
    pub creator_invite: String,
    pub creator_account_idx: usize,
    pub creator_focus: usize,
    pub creator_error: Option<String>,
    pub creator_busy: bool,

    // Room editor overlay state
    pub editor_name: String,
    pub editor_topic: String,
    pub editor_invite_user: String,
    pub editor_focus: usize,
    pub editor_error: Option<String>,
    pub editor_busy: bool,
    pub editor_confirm_leave: bool,
    pub editor_confirm_delete: bool,
    pub editor_room_id: Option<OwnedRoomId>,
    pub editor_account_id: Option<String>,

    // Recovery overlay state
    pub recovery_key: String,
    pub recovery_error: Option<String>,
    pub recovery_busy: bool,
    pub recovery_account_idx: usize,

    // Message selection state
    pub selected_message: Option<usize>,

    // Message action overlay state
    pub message_action_selected: usize, // 0=Edit, 1=Delete
    pub message_editing: bool,
    pub message_edit_text: String,
    pub message_edit_error: Option<String>,
    pub message_edit_busy: bool,

    // Pagination tokens for loading older messages
    pub room_history_tokens: HashMap<OwnedRoomId, Option<String>>,

    // Viewport size (messages that fit on screen), updated during draw
    pub chat_viewport_msgs: Cell<usize>,

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
        let room_sort = RoomSortMode::from_str(&config.room_sort);
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
            downloading_keys: false,
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
            room_sort,
            favorites_count: 0,
            settings_sort_open: false,
            settings_sort_selected: 0,
            profile_display_name: String::new(),
            profile_avatar_url: String::new(),
            profile_avatar_path: String::new(),
            profile_focus: 0,
            profile_error: None,
            profile_busy: false,
            profile_account_idx: 0,
            profile_current_name: String::new(),
            profile_current_avatar: String::new(),
            creator_name: String::new(),
            creator_topic: String::new(),
            creator_visibility: 0,
            creator_e2ee: true,
            creator_federated: true,
            creator_invite: String::new(),
            creator_account_idx: 0,
            creator_focus: 0,
            creator_error: None,
            creator_busy: false,
            editor_name: String::new(),
            editor_topic: String::new(),
            editor_invite_user: String::new(),
            editor_focus: 0,
            editor_error: None,
            editor_busy: false,
            editor_confirm_leave: false,
            editor_confirm_delete: false,
            editor_room_id: None,
            editor_account_id: None,
            recovery_key: String::new(),
            recovery_error: None,
            recovery_busy: false,
            recovery_account_idx: 0,
            selected_message: None,
            message_action_selected: 0,
            message_editing: false,
            message_edit_text: String::new(),
            message_edit_error: None,
            message_edit_busy: false,
            room_history_tokens: HashMap::new(),
            chat_viewport_msgs: Cell::new(10),
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

    fn open_settings(&mut self) {
        self.overlay = Overlay::Settings;
        self.settings_selected = 0;
        self.settings_accounts_open = false;
        self.settings_account_action_open = false;
        self.settings_theme_open = false;
        self.settings_sort_open = false;
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

        // Global shortcuts when no overlay is active and not typing
        if self.overlay == Overlay::None && self.focus != Focus::Input {
            match key.code {
                KeyCode::Char('s') => {
                    self.open_settings();
                    return;
                }
                KeyCode::Char('n') if !self.accounts.is_empty() => {
                    self.open_room_creator();
                    return;
                }
                KeyCode::Char('e') if self.active_room.is_some() => {
                    self.open_room_editor().await;
                    return;
                }
                _ => {}
            }
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
            Overlay::ProfileEditor => self.handle_profile_key(key).await,
            Overlay::RoomCreator => self.handle_creator_key(key).await,
            Overlay::RoomEditor => self.handle_editor_key(key).await,
            Overlay::Recovery => self.handle_recovery_key(key).await,
            Overlay::MessageAction => self.handle_message_action_key(key).await,
            Overlay::None => match self.focus {
                Focus::Accounts => self.handle_accounts_key(key),
                Focus::Rooms => self.handle_rooms_key(key).await,
                Focus::Chat => self.handle_chat_key(key).await,
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

    async fn handle_rooms_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::SHIFT, KeyCode::Up) => {
                self.reorder_favorite_up().await;
            }
            (KeyModifiers::SHIFT, KeyCode::Down) => {
                self.reorder_favorite_down().await;
            }
            (_, KeyCode::Up) => {
                if self.selected_room > 0 {
                    self.selected_room -= 1;
                }
            }
            (_, KeyCode::Down) => {
                if self.selected_room + 1 < self.all_rooms.len() {
                    self.selected_room += 1;
                }
            }
            (_, KeyCode::Enter) => {
                self.open_selected_room().await;
            }
            (_, KeyCode::Tab) => self.focus = Focus::Chat,
            (_, KeyCode::BackTab) => self.focus = Focus::Accounts,
            (_, KeyCode::Left) => self.focus = Focus::Accounts,
            (_, KeyCode::Right) => self.focus = Focus::Chat,
            (_, KeyCode::Char('f')) => {
                self.toggle_favorite().await;
            }
            (_, KeyCode::Char('a')) => {
                self.overlay = Overlay::Login;
                self.login_homeserver = "matrix.org".to_string();
                self.login_username.clear();
                self.login_password.clear();
                self.login_focus = 0;
                self.login_error = None;
            }
            (_, KeyCode::Char('?')) => self.overlay = Overlay::Help,
            _ => {}
        }
    }

    async fn toggle_favorite(&mut self) {
        let room_id = match self.all_rooms.get(self.selected_room) {
            Some(r) => r.id.to_string(),
            None => return,
        };
        if let Some(pos) = self.config.favorites.iter().position(|f| f == &room_id) {
            self.config.favorites.remove(pos);
        } else {
            self.config.favorites.push(room_id);
        }
        let _ = self.config.save();
        self.refresh_rooms().await;
    }

    async fn reorder_favorite_up(&mut self) {
        if self.selected_room == 0 || self.selected_room >= self.favorites_count {
            return;
        }
        // Swap in config.favorites
        let idx = self.selected_room;
        self.config.favorites.swap(idx, idx - 1);
        let _ = self.config.save();
        self.selected_room -= 1;
        self.refresh_rooms().await;
    }

    async fn reorder_favorite_down(&mut self) {
        if self.selected_room + 1 >= self.favorites_count {
            return;
        }
        let idx = self.selected_room;
        self.config.favorites.swap(idx, idx + 1);
        let _ = self.config.save();
        self.selected_room += 1;
        self.refresh_rooms().await;
    }

    // --- Room Creator ---

    fn open_room_creator(&mut self) {
        self.overlay = Overlay::RoomCreator;
        self.creator_name.clear();
        self.creator_topic.clear();
        self.creator_visibility = 0;
        self.creator_e2ee = true;
        self.creator_federated = true;
        self.creator_invite.clear();
        self.creator_account_idx = self
            .accounts
            .iter()
            .position(|a| Some(&a.user_id) == self.active_account_id.as_ref())
            .unwrap_or(0);
        self.creator_focus = 0;
        self.creator_error = None;
        self.creator_busy = false;
    }

    async fn handle_creator_key(&mut self, key: KeyEvent) {
        if self.creator_busy {
            return;
        }
        // Focus: 0=account, 1=name, 2=topic, 3=visibility, 4=e2ee, 5=federated, 6=invite
        match key.code {
            KeyCode::Tab => {
                self.creator_focus = (self.creator_focus + 1) % 7;
            }
            KeyCode::BackTab => {
                self.creator_focus = if self.creator_focus == 0 { 6 } else { self.creator_focus - 1 };
            }
            KeyCode::Enter => {
                match self.creator_focus {
                    0 if self.accounts.len() > 1 => {
                        self.creator_account_idx = (self.creator_account_idx + 1) % self.accounts.len();
                    }
                    3 => self.creator_visibility = 1 - self.creator_visibility,
                    4 => self.creator_e2ee = !self.creator_e2ee,
                    5 => self.creator_federated = !self.creator_federated,
                    _ => self.do_create_room().await,
                }
            }
            KeyCode::Left if self.creator_focus == 0 && self.accounts.len() > 1 => {
                self.creator_account_idx = if self.creator_account_idx == 0 {
                    self.accounts.len() - 1
                } else {
                    self.creator_account_idx - 1
                };
            }
            KeyCode::Right if self.creator_focus == 0 && self.accounts.len() > 1 => {
                self.creator_account_idx = (self.creator_account_idx + 1) % self.accounts.len();
            }
            KeyCode::Char(' ') if self.creator_focus == 0 && self.accounts.len() > 1 => {
                self.creator_account_idx = (self.creator_account_idx + 1) % self.accounts.len();
            }
            KeyCode::Char(' ') if self.creator_focus == 3 => {
                self.creator_visibility = 1 - self.creator_visibility;
            }
            KeyCode::Char(' ') if self.creator_focus == 4 => {
                self.creator_e2ee = !self.creator_e2ee;
            }
            KeyCode::Char(' ') if self.creator_focus == 5 => {
                self.creator_federated = !self.creator_federated;
            }
            KeyCode::Esc => {
                self.overlay = Overlay::None;
            }
            KeyCode::Char(c) => {
                match self.creator_focus {
                    1 => self.creator_name.push(c),
                    2 => self.creator_topic.push(c),
                    6 => self.creator_invite.push(c),
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                match self.creator_focus {
                    1 => { self.creator_name.pop(); }
                    2 => { self.creator_topic.pop(); }
                    6 => { self.creator_invite.pop(); }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    async fn do_create_room(&mut self) {
        if self.creator_name.is_empty() {
            self.creator_error = Some("Room name is required".to_string());
            return;
        }
        let account_idx = self.creator_account_idx;
        if account_idx >= self.accounts.len() {
            self.creator_error = Some("No account available".to_string());
            return;
        }

        self.creator_busy = true;
        self.creator_error = None;

        let invite_ids: Vec<String> = self
            .creator_invite
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let name = Some(self.creator_name.as_str());
        let topic = if self.creator_topic.is_empty() {
            None
        } else {
            Some(self.creator_topic.as_str())
        };
        let is_public = self.creator_visibility == 1;

        match self.accounts[account_idx]
            .create_room(name, topic, is_public, self.creator_e2ee, invite_ids)
            .await
        {
            Ok(room_id) => {
                self.status_msg = format!("Created room: {}", self.creator_name);
                self.overlay = Overlay::None;
                self.refresh_rooms().await;
                if let Some(idx) = self.all_rooms.iter().position(|r| r.id == room_id) {
                    self.selected_room = idx;
                    self.open_selected_room().await;
                }
            }
            Err(e) => {
                self.creator_error = Some(e.to_string());
            }
        }
        self.creator_busy = false;
    }

    // --- Room Editor ---

    async fn open_room_editor(&mut self) {
        if let (Some(room_id), Some(account_id)) =
            (self.active_room.clone(), self.active_account_id.clone())
        {
            let current_name = self
                .all_rooms
                .iter()
                .find(|r| r.id == room_id)
                .map(|r| r.name.clone())
                .unwrap_or_default();
            let current_topic = self
                .accounts
                .iter()
                .find(|a| a.user_id == account_id)
                .and_then(|acct| acct.get_room_topic(&room_id))
                .unwrap_or_default();

            self.overlay = Overlay::RoomEditor;
            self.editor_name = current_name;
            self.editor_topic = current_topic;
            self.editor_invite_user.clear();
            self.editor_focus = 0;
            self.editor_error = None;
            self.editor_busy = false;
            self.editor_confirm_leave = false;
            self.editor_confirm_delete = false;
            self.editor_room_id = Some(room_id);
            self.editor_account_id = Some(account_id);
        }
    }

    async fn handle_editor_key(&mut self, key: KeyEvent) {
        if self.editor_busy {
            return;
        }
        // Focus: 0=name, 1=topic, 2=invite, 3=leave, 4=delete
        match key.code {
            KeyCode::Tab => {
                self.editor_focus = (self.editor_focus + 1) % 5;
                self.editor_confirm_leave = false;
                self.editor_confirm_delete = false;
            }
            KeyCode::BackTab => {
                self.editor_focus = if self.editor_focus == 0 { 4 } else { self.editor_focus - 1 };
                self.editor_confirm_leave = false;
                self.editor_confirm_delete = false;
            }
            KeyCode::Enter => {
                match self.editor_focus {
                    0 => self.do_edit_room_name().await,
                    1 => self.do_edit_room_topic().await,
                    2 => self.do_invite_user().await,
                    3 => {
                        if self.editor_confirm_leave {
                            self.do_leave_room().await;
                        } else {
                            self.editor_confirm_leave = true;
                        }
                    }
                    4 => {
                        if self.editor_confirm_delete {
                            self.do_delete_room().await;
                        } else {
                            self.editor_confirm_delete = true;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Esc => {
                if self.editor_confirm_leave || self.editor_confirm_delete {
                    self.editor_confirm_leave = false;
                    self.editor_confirm_delete = false;
                } else {
                    self.overlay = Overlay::None;
                }
            }
            KeyCode::Char(c) => {
                self.editor_confirm_leave = false;
                self.editor_confirm_delete = false;
                match self.editor_focus {
                    0 => self.editor_name.push(c),
                    1 => self.editor_topic.push(c),
                    2 => self.editor_invite_user.push(c),
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                self.editor_confirm_leave = false;
                self.editor_confirm_delete = false;
                match self.editor_focus {
                    0 => { self.editor_name.pop(); }
                    1 => { self.editor_topic.pop(); }
                    2 => { self.editor_invite_user.pop(); }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    async fn do_edit_room_name(&mut self) {
        let (room_id, account_id) = match (&self.editor_room_id, &self.editor_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };
        if self.editor_name.is_empty() {
            self.editor_error = Some("Name cannot be empty".to_string());
            return;
        }
        self.editor_busy = true;
        self.editor_error = None;
        if let Some(acct) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match acct.set_room_name(&room_id, &self.editor_name).await {
                Ok(()) => {
                    self.status_msg = "Room name updated".to_string();
                    self.refresh_rooms().await;
                }
                Err(e) => self.editor_error = Some(e.to_string()),
            }
        }
        self.editor_busy = false;
    }

    async fn do_edit_room_topic(&mut self) {
        let (room_id, account_id) = match (&self.editor_room_id, &self.editor_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };
        self.editor_busy = true;
        self.editor_error = None;
        if let Some(acct) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match acct.set_room_topic(&room_id, &self.editor_topic).await {
                Ok(()) => self.status_msg = "Room topic updated".to_string(),
                Err(e) => self.editor_error = Some(e.to_string()),
            }
        }
        self.editor_busy = false;
    }

    async fn do_invite_user(&mut self) {
        let (room_id, account_id) = match (&self.editor_room_id, &self.editor_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };
        if self.editor_invite_user.trim().is_empty() {
            self.editor_error = Some("Enter a user ID".to_string());
            return;
        }
        self.editor_busy = true;
        self.editor_error = None;
        if let Some(acct) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match acct.invite_user(&room_id, self.editor_invite_user.trim()).await {
                Ok(()) => {
                    self.status_msg = format!("Invited {}", self.editor_invite_user.trim());
                    self.editor_invite_user.clear();
                }
                Err(e) => self.editor_error = Some(e.to_string()),
            }
        }
        self.editor_busy = false;
    }

    async fn do_leave_room(&mut self) {
        let (room_id, account_id) = match (&self.editor_room_id, &self.editor_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };
        self.editor_busy = true;
        self.editor_error = None;
        if let Some(acct) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match acct.leave_room(&room_id).await {
                Ok(()) => {
                    self.status_msg = format!("Left room");
                    self.active_room = None;
                    self.active_account_id = None;
                    self.messages.clear();
                    self.overlay = Overlay::None;
                    self.refresh_rooms().await;
                }
                Err(e) => self.editor_error = Some(e.to_string()),
            }
        }
        self.editor_busy = false;
    }

    async fn do_delete_room(&mut self) {
        let (room_id, account_id) = match (&self.editor_room_id, &self.editor_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };
        self.editor_busy = true;
        self.editor_error = None;
        if let Some(acct) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match acct.forget_room(&room_id).await {
                Ok(()) => {
                    self.status_msg = "Room deleted".to_string();
                    self.active_room = None;
                    self.active_account_id = None;
                    self.messages.clear();
                    self.room_messages.remove(&room_id);
                    self.overlay = Overlay::None;
                    self.refresh_rooms().await;
                }
                Err(e) => self.editor_error = Some(e.to_string()),
            }
        }
        self.editor_busy = false;
    }

    // --- Profile Editor ---

    async fn open_profile_editor(&mut self, account_idx: usize) {
        if account_idx >= self.accounts.len() {
            return;
        }
        self.profile_account_idx = account_idx;
        self.profile_busy = true;
        self.overlay = Overlay::ProfileEditor;
        self.profile_focus = 0;
        self.profile_error = None;
        self.profile_avatar_url.clear();
        self.profile_avatar_path.clear();

        let acct = &self.accounts[account_idx];
        self.profile_current_name = acct
            .get_display_name()
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| "(not set)".to_string());
        self.profile_current_avatar = acct
            .get_avatar_url()
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| "(not set)".to_string());

        self.profile_display_name = if self.profile_current_name == "(not set)" {
            String::new()
        } else {
            self.profile_current_name.clone()
        };
        self.profile_busy = false;
    }

    async fn handle_profile_key(&mut self, key: KeyEvent) {
        if self.profile_busy {
            return;
        }
        match key.code {
            KeyCode::Tab => {
                self.profile_focus = (self.profile_focus + 1) % 3;
            }
            KeyCode::BackTab => {
                self.profile_focus = if self.profile_focus == 0 { 2 } else { self.profile_focus - 1 };
            }
            KeyCode::Enter => {
                match self.profile_focus {
                    0 => self.do_set_display_name().await,
                    1 => self.do_set_avatar_url().await,
                    2 => self.do_upload_avatar().await,
                    _ => {}
                }
            }
            KeyCode::Esc => {
                self.overlay = Overlay::None;
            }
            KeyCode::Char(c) => {
                match self.profile_focus {
                    0 => self.profile_display_name.push(c),
                    1 => self.profile_avatar_url.push(c),
                    2 => self.profile_avatar_path.push(c),
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                match self.profile_focus {
                    0 => { self.profile_display_name.pop(); }
                    1 => { self.profile_avatar_url.pop(); }
                    2 => { self.profile_avatar_path.pop(); }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    async fn do_set_display_name(&mut self) {
        let idx = self.profile_account_idx;
        if idx >= self.accounts.len() || self.profile_display_name.is_empty() {
            return;
        }
        self.profile_busy = true;
        self.profile_error = None;
        match self.accounts[idx].set_display_name(&self.profile_display_name).await {
            Ok(()) => {
                self.profile_current_name = self.profile_display_name.clone();
                self.accounts[idx].display_name = self.profile_display_name.clone();
                self.status_msg = "Display name updated".to_string();
                self.overlay = Overlay::None;
            }
            Err(e) => self.profile_error = Some(e.to_string()),
        }
        self.profile_busy = false;
    }

    async fn do_set_avatar_url(&mut self) {
        let idx = self.profile_account_idx;
        if idx >= self.accounts.len() || self.profile_avatar_url.is_empty() {
            return;
        }
        self.profile_busy = true;
        self.profile_error = None;
        match self.accounts[idx].set_avatar_url(&self.profile_avatar_url).await {
            Ok(()) => {
                self.profile_current_avatar = self.profile_avatar_url.clone();
                self.status_msg = "Avatar URL updated".to_string();
                self.overlay = Overlay::None;
            }
            Err(e) => self.profile_error = Some(e.to_string()),
        }
        self.profile_busy = false;
    }

    async fn do_upload_avatar(&mut self) {
        let idx = self.profile_account_idx;
        if idx >= self.accounts.len() || self.profile_avatar_path.is_empty() {
            return;
        }
        self.profile_busy = true;
        self.profile_error = None;
        match self.accounts[idx].upload_avatar(&self.profile_avatar_path).await {
            Ok(mxc_url) => {
                self.profile_current_avatar = mxc_url;
                self.status_msg = "Avatar uploaded".to_string();
                self.overlay = Overlay::None;
            }
            Err(e) => self.profile_error = Some(e.to_string()),
        }
        self.profile_busy = false;
    }

    fn open_recovery(&mut self, account_idx: usize) {
        self.recovery_account_idx = account_idx;
        self.recovery_key.clear();
        self.recovery_error = None;
        self.recovery_busy = false;
        self.overlay = Overlay::Recovery;
    }

    async fn handle_recovery_key(&mut self, key: KeyEvent) {
        if self.recovery_busy {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                if !self.recovery_key.is_empty() {
                    self.do_recover().await;
                }
            }
            KeyCode::Char(c) => {
                self.recovery_key.push(c);
            }
            KeyCode::Backspace => {
                self.recovery_key.pop();
            }
            KeyCode::Esc => {
                self.overlay = Overlay::Settings;
            }
            _ => {}
        }
    }

    async fn do_recover(&mut self) {
        let idx = self.recovery_account_idx;
        if idx >= self.accounts.len() {
            return;
        }
        self.recovery_busy = true;
        self.recovery_error = None;
        let key = self.recovery_key.trim().to_string();
        match self.accounts[idx].recover_with_key(&key).await {
            Ok(()) => {
                let user_id = &self.accounts[idx].user_id;
                self.status_msg = format!("Session verified for {}", user_id);
                self.overlay = Overlay::None;
            }
            Err(e) => {
                self.recovery_error = Some(e.to_string());
            }
        }
        self.recovery_busy = false;
    }

    // --- Message Actions ---

    fn open_message_action(&mut self) {
        if let Some(idx) = self.selected_message {
            if idx < self.messages.len() {
                self.message_action_selected = 0;
                self.message_editing = false;
                self.message_edit_text = self.messages[idx].body.clone();
                self.message_edit_error = None;
                self.message_edit_busy = false;
                self.overlay = Overlay::MessageAction;
            }
        }
    }

    async fn handle_message_action_key(&mut self, key: KeyEvent) {
        if self.message_edit_busy {
            return;
        }

        if self.message_editing {
            // In edit text mode
            match key.code {
                KeyCode::Enter => {
                    self.do_edit_message().await;
                }
                KeyCode::Esc => {
                    self.message_editing = false;
                    self.message_edit_error = None;
                }
                KeyCode::Char(c) => {
                    self.message_edit_text.push(c);
                }
                KeyCode::Backspace => {
                    self.message_edit_text.pop();
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Up => {
                self.message_action_selected = self.message_action_selected.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.message_action_selected < 1 {
                    self.message_action_selected += 1;
                }
            }
            KeyCode::Enter => {
                match self.message_action_selected {
                    0 => {
                        // Edit — open text editor
                        if let Some(idx) = self.selected_message {
                            if let Some(msg) = self.messages.get(idx) {
                                if msg.event_id.is_none() {
                                    self.message_edit_error =
                                        Some("Cannot edit: no event ID".to_string());
                                    return;
                                }
                                self.message_editing = true;
                                self.message_edit_error = None;
                            }
                        }
                    }
                    1 => {
                        // Delete
                        self.do_delete_message().await;
                    }
                    _ => {}
                }
            }
            KeyCode::Esc => {
                self.overlay = Overlay::None;
            }
            _ => {}
        }
    }

    async fn do_edit_message(&mut self) {
        let msg_idx = match self.selected_message {
            Some(idx) => idx,
            None => return,
        };
        let msg = match self.messages.get(msg_idx) {
            Some(m) => m.clone(),
            None => return,
        };
        let event_id = match &msg.event_id {
            Some(id) => id.clone(),
            None => {
                self.message_edit_error = Some("Cannot edit: no event ID".to_string());
                return;
            }
        };
        let (room_id, account_id) = match (&self.active_room, &self.active_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };

        self.message_edit_busy = true;
        self.message_edit_error = None;

        if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match account
                .edit_message(&room_id, &event_id, &self.message_edit_text)
                .await
            {
                Ok(()) => {
                    // Update local message
                    if let Some(m) = self.messages.get_mut(msg_idx) {
                        m.body = self.message_edit_text.clone();
                    }
                    self.overlay = Overlay::None;
                    self.status_msg = "Message edited".to_string();
                }
                Err(e) => {
                    self.message_edit_error = Some(e.to_string());
                }
            }
        }
        self.message_edit_busy = false;
    }

    async fn do_delete_message(&mut self) {
        let msg_idx = match self.selected_message {
            Some(idx) => idx,
            None => return,
        };
        let msg = match self.messages.get(msg_idx) {
            Some(m) => m.clone(),
            None => return,
        };
        let event_id = match &msg.event_id {
            Some(id) => id.clone(),
            None => {
                self.message_edit_error = Some("Cannot delete: no event ID".to_string());
                return;
            }
        };
        let (room_id, account_id) = match (&self.active_room, &self.active_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };

        self.message_edit_busy = true;
        self.message_edit_error = None;

        if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match account.redact_message(&room_id, &event_id).await {
                Ok(()) => {
                    self.messages.remove(msg_idx);
                    // Adjust selected_message
                    if self.messages.is_empty() {
                        self.selected_message = None;
                    } else if msg_idx >= self.messages.len() {
                        self.selected_message = Some(self.messages.len() - 1);
                    }
                    self.overlay = Overlay::None;
                    self.status_msg = "Message deleted".to_string();
                }
                Err(e) => {
                    self.message_edit_error = Some(e.to_string());
                }
            }
        }
        self.message_edit_busy = false;
    }

    async fn fetch_older_messages(&mut self) {
        let (room_id, account_id) = match (&self.active_room, &self.active_account_id) {
            (Some(r), Some(a)) => (r.clone(), a.clone()),
            _ => return,
        };
        let token = match self.room_history_tokens.get(&room_id) {
            Some(Some(t)) => t.clone(),
            _ => return, // no more history or no token stored
        };

        self.status_msg = "Loading older messages...".to_string();

        if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
            match account
                .fetch_history_paged(&room_id, Some(&token), 50)
                .await
            {
                Ok((mut older_msgs, next_token)) => {
                    if older_msgs.is_empty() {
                        self.room_history_tokens.insert(room_id, None);
                        self.status_msg = "No more messages".to_string();
                        return;
                    }
                    let count = older_msgs.len();
                    // Prepend older messages
                    older_msgs.append(&mut self.messages);
                    self.messages = older_msgs;
                    // Adjust selected_message and scroll_offset for the prepended messages
                    if let Some(sel) = self.selected_message {
                        self.selected_message = Some(sel + count);
                    }
                    self.scroll_offset += count;
                    // Store next token for further pagination
                    self.room_history_tokens.insert(room_id, next_token);
                    self.status_msg = format!("Loaded {} older messages", count);
                }
                Err(e) => {
                    self.status_msg = format!("Failed to load history: {}", e);
                }
            }
        }
    }

    async fn handle_chat_key(&mut self, key: KeyEvent) {
        let viewport = self.chat_viewport_msgs.get().max(1);

        match key.code {
            KeyCode::Up => {
                if self.messages.is_empty() {
                    return;
                }
                match self.selected_message {
                    None => {
                        // Start selecting from the bottom
                        self.selected_message = Some(self.messages.len() - 1);
                        self.scroll_offset = 0;
                    }
                    Some(0) => {
                        // At top — try to load older messages
                        self.fetch_older_messages().await;
                    }
                    Some(idx) => {
                        let new_idx = idx - 1;
                        self.selected_message = Some(new_idx);
                        // Only scroll if selection would go above the visible area
                        let end = self.messages.len().saturating_sub(self.scroll_offset);
                        let start = end.saturating_sub(viewport);
                        if new_idx < start {
                            self.scroll_offset = self.scroll_offset.saturating_add(1);
                        }
                    }
                }
            }
            KeyCode::Down => {
                match self.selected_message {
                    Some(idx) if idx + 1 < self.messages.len() => {
                        let new_idx = idx + 1;
                        self.selected_message = Some(new_idx);
                        // Only scroll if selection would go below the visible area
                        let end = self.messages.len().saturating_sub(self.scroll_offset);
                        if new_idx >= end {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                    }
                    Some(_) => {
                        // At bottom — deselect, return to live view
                        self.selected_message = None;
                        self.scroll_offset = 0;
                    }
                    None => {}
                }
            }
            KeyCode::Enter => {
                if self.selected_message.is_some() {
                    self.open_message_action();
                }
            }
            KeyCode::Home => {
                if !self.messages.is_empty() {
                    self.selected_message = Some(0);
                    self.scroll_offset = self.messages.len().saturating_sub(viewport);
                }
            }
            KeyCode::End => {
                self.selected_message = None;
                self.scroll_offset = 0;
            }
            KeyCode::Tab => self.focus = Focus::Input,
            KeyCode::BackTab => self.focus = Focus::Rooms,
            KeyCode::Left => self.focus = Focus::Rooms,
            KeyCode::Esc => {
                if self.selected_message.is_some() {
                    self.selected_message = None;
                    self.scroll_offset = 0;
                } else {
                    self.focus = Focus::Rooms;
                }
            }
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
                } else if self.settings_sort_open {
                    self.settings_sort_open = false;
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
                } else if self.settings_sort_open {
                    self.settings_sort_selected =
                        self.settings_sort_selected.saturating_sub(1);
                } else {
                    self.settings_selected = self.settings_selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if self.settings_account_action_open {
                    if self.settings_account_action_selected < 3 {
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
                } else if self.settings_sort_open {
                    if self.settings_sort_selected + 1 < RoomSortMode::ALL.len() {
                        self.settings_sort_selected += 1;
                    }
                } else if self.settings_selected < 3 {
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
                        2 => {
                            // Edit Profile
                            self.settings_account_action_open = false;
                            self.open_profile_editor(acct_idx).await;
                        }
                        3 => {
                            // Verify Session
                            self.settings_account_action_open = false;
                            self.open_recovery(acct_idx);
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
                } else if self.settings_sort_open {
                    if let Some(&mode) = RoomSortMode::ALL.get(self.settings_sort_selected) {
                        self.room_sort = mode;
                        self.config.room_sort = mode.as_str().to_string();
                        let _ = self.config.save();
                        self.refresh_rooms().await;
                    }
                    self.settings_sort_open = false;
                } else if self.settings_selected == 0 {
                    // Open accounts sub-menu
                    self.settings_accounts_open = true;
                    self.settings_theme_open = false;
                    self.settings_sort_open = false;
                    self.settings_accounts_selected = 0;
                } else if self.settings_selected == 1 {
                    // Open theme picker
                    self.settings_theme_open = true;
                    self.settings_accounts_open = false;
                    self.settings_sort_open = false;
                    let themes = ui::builtin_themes();
                    self.settings_theme_selected = themes
                        .iter()
                        .position(|t| t.name == self.theme.name)
                        .unwrap_or(0);
                } else if self.settings_selected == 2 {
                    // Open sort picker
                    self.settings_sort_open = true;
                    self.settings_accounts_open = false;
                    self.settings_theme_open = false;
                    self.settings_sort_selected = RoomSortMode::ALL
                        .iter()
                        .position(|m| m == &self.room_sort)
                        .unwrap_or(0);
                } else if self.settings_selected == 3 {
                    // Clear Cache
                    self.do_clear_cache();
                    self.overlay = Overlay::None;
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

        // Stop sync and remove old account
        self.accounts[idx].stop_sync();
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

        // Stop sync and remove from active accounts
        self.accounts[idx].stop_sync();
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

    fn do_clear_cache(&mut self) {
        let sessions_dir = crate::config::data_dir().join("sessions");
        if sessions_dir.exists() {
            match std::fs::remove_dir_all(&sessions_dir) {
                Ok(_) => self.status_msg = "Cache cleared".to_string(),
                Err(e) => self.status_msg = format!("Failed to clear cache: {}", e),
            }
        } else {
            self.status_msg = "No cache to clear".to_string();
        }
    }

    async fn do_login(&mut self) {
        self.login_busy = true;
        self.login_error = None;

        // Check if already logged in to this homeserver with this username
        let user = self.login_username.trim();
        let hs = self.login_homeserver.trim();
        let check_id = format!("@{}:{}", user, hs);
        let check_id_stripped = format!("@{}:{}", user.trim_start_matches('@'), hs);
        if self.accounts.iter().any(|a| {
            a.user_id == check_id
                || a.user_id == check_id_stripped
                || a.user_id == user
                || a.homeserver == hs && a.user_id.starts_with(&format!("@{}:", user.trim_start_matches('@')))
        }) {
            self.login_error = Some("Already logged in — use Verify Session to recover E2EE keys".to_string());
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
                        event_id: None, // filled in when sync returns the event
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
                event_id,
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
                    event_id: Some(event_id),
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
            MatrixEvent::KeysDownloaded { room_id, account_id } => {
                self.downloading_keys = false;
                // Re-fetch history if we're still viewing this room
                if self.active_room.as_ref() == Some(&room_id)
                    && self.active_account_id.as_deref() == Some(&account_id)
                {
                    if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
                        match account.fetch_history(&room_id, 50).await {
                            Ok(msgs) if !msgs.is_empty() => {
                                let count = msgs.len();
                                let decrypted = msgs.iter().filter(|m| !m.body.contains("[encrypted message")).count();
                                self.messages = msgs;
                                self.status_msg = format!("Decrypted {}/{} messages", decrypted, count);
                            }
                            _ => {}
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
        // Remember current selection by room ID
        let prev_id = self.all_rooms.get(self.selected_room).map(|r| r.id.clone());

        let mut all: Vec<RoomInfo> = Vec::new();
        for account in &self.accounts {
            all.extend(account.rooms().await);
        }

        // Partition into favorites (ordered by config) and others
        let mut favorites: Vec<RoomInfo> = Vec::new();
        for fav_id in &self.config.favorites {
            if let Some(pos) = all.iter().position(|r| r.id.as_str() == fav_id) {
                favorites.push(all.remove(pos));
            }
        }

        // Sort the remaining rooms
        self.sort_rooms(&mut all);

        self.favorites_count = favorites.len();
        self.all_rooms = favorites;
        self.all_rooms.append(&mut all);

        // Restore selection by room ID
        if let Some(prev) = prev_id {
            if let Some(idx) = self.all_rooms.iter().position(|r| r.id == prev) {
                self.selected_room = idx;
            }
        }
        // Clamp
        if self.selected_room >= self.all_rooms.len() && !self.all_rooms.is_empty() {
            self.selected_room = self.all_rooms.len() - 1;
        }
    }

    fn sort_rooms(&self, rooms: &mut Vec<RoomInfo>) {
        match self.room_sort {
            RoomSortMode::Unread => {
                rooms.sort_by(|a, b| {
                    b.unread
                        .cmp(&a.unread)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            RoomSortMode::Recent => {
                rooms.sort_by(|a, b| {
                    let ts_a = self
                        .room_messages
                        .get(&a.id)
                        .and_then(|msgs| msgs.last())
                        .map(|m| m.timestamp)
                        .unwrap_or(0);
                    let ts_b = self
                        .room_messages
                        .get(&b.id)
                        .and_then(|msgs| msgs.last())
                        .map(|m| m.timestamp)
                        .unwrap_or(0);
                    ts_b.cmp(&ts_a)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            RoomSortMode::Alpha => {
                rooms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
        }
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
            self.selected_message = None;
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

            // Try fetch_history first (with pagination token)
            if let Some(account) = self.accounts.iter().find(|a| a.user_id == account_id) {
                match account.fetch_history_paged(&room_id, None, 50).await {
                    Ok((msgs, end_token)) if !msgs.is_empty() => {
                        let count = msgs.len();
                        self.room_history_tokens.insert(room_id.clone(), end_token);
                        let has_encrypted = msgs.iter().any(|m| m.body.contains("[encrypted message"));
                        self.messages = msgs;
                        if has_encrypted {
                            // Encrypted messages found — SDK will auto-download keys
                            // Schedule a delayed re-fetch to pick up decrypted content
                            self.downloading_keys = true;
                            self.status_msg = format!(
                                "{} — downloading room keys...",
                                room_name
                            );
                            let tx = self.matrix_tx.clone();
                            let rid = room_id.clone();
                            let aid = account_id.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                                let _ = tx.send(MatrixEvent::KeysDownloaded {
                                    room_id: rid,
                                    account_id: aid,
                                });
                            });
                        } else {
                            self.status_msg = format!(
                                "{} ({}) — {} messages",
                                room_name, account_id, count
                            );
                        }
                    }
                    Ok((_, _)) => {
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
