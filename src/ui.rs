use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, Focus, Overlay, RoomSortMode};

// --- Theme system ---

#[derive(Clone)]
pub struct Theme {
    pub name: &'static str,
    pub accent: Color,
    pub dimmed: Color,
    pub text: Color,
    pub text_dim: Color,
    pub status_ok: Color,
    pub status_err: Color,
    pub status_warn: Color,
    pub status_bg: Color,
    pub highlight_bg: Color,
}

pub fn builtin_themes() -> Vec<Theme> {
    vec![
        Theme {
            name: "Default",
            accent: Color::Cyan,
            dimmed: Color::DarkGray,
            text: Color::White,
            text_dim: Color::Gray,
            status_ok: Color::Green,
            status_err: Color::Red,
            status_warn: Color::Yellow,
            status_bg: Color::Black,
            highlight_bg: Color::DarkGray,
        },
        Theme {
            name: "Dracula",
            accent: Color::Rgb(189, 147, 249),
            dimmed: Color::Rgb(98, 114, 164),
            text: Color::Rgb(248, 248, 242),
            text_dim: Color::Rgb(189, 189, 189),
            status_ok: Color::Rgb(80, 250, 123),
            status_err: Color::Rgb(255, 85, 85),
            status_warn: Color::Rgb(241, 250, 140),
            status_bg: Color::Rgb(40, 42, 54),
            highlight_bg: Color::Rgb(68, 71, 90),
        },
        Theme {
            name: "Gruvbox",
            accent: Color::Rgb(254, 128, 25),
            dimmed: Color::Rgb(102, 92, 84),
            text: Color::Rgb(235, 219, 178),
            text_dim: Color::Rgb(168, 153, 132),
            status_ok: Color::Rgb(184, 187, 38),
            status_err: Color::Rgb(251, 73, 52),
            status_warn: Color::Rgb(250, 189, 47),
            status_bg: Color::Rgb(40, 40, 40),
            highlight_bg: Color::Rgb(60, 56, 54),
        },
        Theme {
            name: "Nord",
            accent: Color::Rgb(136, 192, 208),
            dimmed: Color::Rgb(76, 86, 106),
            text: Color::Rgb(236, 239, 244),
            text_dim: Color::Rgb(216, 222, 233),
            status_ok: Color::Rgb(163, 190, 140),
            status_err: Color::Rgb(191, 97, 106),
            status_warn: Color::Rgb(235, 203, 139),
            status_bg: Color::Rgb(46, 52, 64),
            highlight_bg: Color::Rgb(59, 66, 82),
        },
        Theme {
            name: "Monokai",
            accent: Color::Rgb(166, 226, 46),
            dimmed: Color::Rgb(117, 113, 94),
            text: Color::Rgb(248, 248, 242),
            text_dim: Color::Rgb(191, 191, 191),
            status_ok: Color::Rgb(166, 226, 46),
            status_err: Color::Rgb(249, 38, 114),
            status_warn: Color::Rgb(230, 219, 116),
            status_bg: Color::Rgb(39, 40, 34),
            highlight_bg: Color::Rgb(62, 61, 50),
        },
        Theme {
            name: "Solarized",
            accent: Color::Rgb(38, 139, 210),
            dimmed: Color::Rgb(88, 110, 117),
            text: Color::Rgb(131, 148, 150),
            text_dim: Color::Rgb(101, 123, 131),
            status_ok: Color::Rgb(133, 153, 0),
            status_err: Color::Rgb(220, 50, 47),
            status_warn: Color::Rgb(181, 137, 0),
            status_bg: Color::Rgb(0, 43, 54),
            highlight_bg: Color::Rgb(7, 54, 66),
        },
    ]
}

pub fn theme_by_name(name: &str) -> Theme {
    let themes = builtin_themes();
    let idx = themes
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name))
        .unwrap_or(0);
    themes.into_iter().nth(idx).unwrap()
}

// --- Drawing ---

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let width = size.width;

    // Decide layout based on terminal width
    if width >= 120 {
        draw_three_column(f, app, size);
    } else if width >= 60 {
        draw_two_column(f, app, size);
    } else {
        draw_single_column(f, app, size);
    }

    // Draw overlays on top
    match app.overlay {
        Overlay::Login => draw_login_overlay(f, app),
        Overlay::Help => draw_help_overlay(f, &app.theme),
        Overlay::RoomSwitcher => draw_switcher_overlay(f, app),
        Overlay::Settings => draw_settings_overlay(f, app),
        Overlay::None => {}
    }
}

fn draw_three_column(f: &mut Frame, app: &App, area: Rect) {
    // Top area and bottom status bar
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let main_area = vertical[0];
    let status_area = vertical[1];

    // Three columns
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),
            Constraint::Length(24),
            Constraint::Min(30),
        ])
        .split(main_area);

    draw_accounts_panel(f, app, columns[0]);
    draw_rooms_panel(f, app, columns[1]);
    draw_chat_panel(f, app, columns[2]);
    draw_status_bar(f, app, status_area);
}

fn draw_two_column(f: &mut Frame, app: &App, area: Rect) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(30)])
        .split(vertical[0]);

    draw_rooms_panel(f, app, columns[0]);
    draw_chat_panel(f, app, columns[1]);
    draw_status_bar(f, app, vertical[1]);
}

fn draw_single_column(f: &mut Frame, app: &App, area: Rect) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    match app.focus {
        Focus::Accounts | Focus::Rooms => draw_rooms_panel(f, app, vertical[0]),
        _ => draw_chat_panel(f, app, vertical[0]),
    }
    draw_status_bar(f, app, vertical[1]);
}

fn draw_accounts_panel(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Accounts;
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dimmed)
    };
    let block = Block::default()
        .title(" Accounts ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut items: Vec<ListItem> = app
        .accounts
        .iter()
        .enumerate()
        .map(|(i, acct)| {
            let marker = if acct.syncing { "●" } else { "○" };
            // Show short homeserver name
            let label = &acct.homeserver;
            let style = if i == app.selected_account {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(" {} {}", marker, label)).style(style)
        })
        .collect();

    items.push(ListItem::new("").style(Style::default()));
    items.push(
        ListItem::new(" [a] Add")
            .style(Style::default().fg(theme.dimmed)),
    );

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_rooms_panel(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Rooms;
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dimmed)
    };
    let block = Block::default()
        .title(" Rooms ")
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.all_rooms.is_empty() {
        let empty = Paragraph::new(" No rooms yet\n\n Press 'a' to\n add an account")
            .style(Style::default().fg(theme.dimmed))
            .block(block);
        f.render_widget(empty, area);
        return;
    }

    let has_separator = app.favorites_count > 0
        && app.favorites_count < app.all_rooms.len();

    let mut items: Vec<ListItem> = Vec::new();
    // Track mapping from visual index -> all_rooms index
    // The separator is visual-only and not in all_rooms
    let mut visual_to_room: Vec<Option<usize>> = Vec::new();

    for (i, room) in app.all_rooms.iter().enumerate() {
        // Insert separator between favorites and others
        if has_separator && i == app.favorites_count {
            let sep_width = (area.width as usize).saturating_sub(2);
            items.push(
                ListItem::new(format!(" {}", "\u{2500}".repeat(sep_width.saturating_sub(1))))
                    .style(Style::default().fg(theme.dimmed)),
            );
            visual_to_room.push(None);
        }

        let is_fav = i < app.favorites_count;
        let prefix = if is_fav {
            "\u{2605}"
        } else if room.is_dm {
            "@"
        } else {
            "#"
        };
        let unread = if room.unread > 0 {
            format!(" ({})", room.unread)
        } else {
            String::new()
        };

        let is_active = Some(&room.id) == app.active_room.as_ref();
        let is_selected = i == app.selected_room;

        let style = if is_active {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_selected && focused {
            Style::default().fg(theme.text).bg(theme.highlight_bg)
        } else if room.unread > 0 {
            Style::default()
                .fg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };

        // Truncate name to fit
        let max_name = (area.width as usize).saturating_sub(6);
        let name = if room.name.len() > max_name {
            format!("{}…", &room.name[..max_name.saturating_sub(1)])
        } else {
            room.name.clone()
        };

        items.push(ListItem::new(format!(" {}{}{}", prefix, name, unread)).style(style));
        visual_to_room.push(Some(i));
    }

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_chat_panel(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Chat || app.focus == Focus::Input;
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dimmed)
    };

    let title = if let Some(room_id) = &app.active_room {
        if let Some(room) = app.all_rooms.iter().find(|r| &r.id == room_id) {
            format!(" {} · {} ", room.name, room.account_id)
        } else {
            " Chat ".to_string()
        }
    } else {
        " Chat ".to_string()
    };

    // Split chat area: messages + input
    let chat_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let msg_area = chat_layout[0];
    let input_area = chat_layout[1];

    // Messages
    let msg_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.messages.is_empty() && app.active_room.is_none() {
        let welcome = Paragraph::new("\n  Select a room to start chatting\n\n  Ctrl+K  quick room switcher\n  a       add account\n  s       settings\n  ?       help")
            .style(Style::default().fg(theme.dimmed))
            .block(msg_block);
        f.render_widget(welcome, msg_area);
    } else {
        let msg_height = msg_area.height.saturating_sub(2) as usize;
        let start = if app.messages.len() > msg_height + app.scroll_offset {
            app.messages.len() - msg_height - app.scroll_offset
        } else {
            0
        };
        let end = if app.messages.len() > app.scroll_offset {
            app.messages.len() - app.scroll_offset
        } else {
            app.messages.len()
        };

        let visible: Vec<Line> = app.messages[start..end]
            .iter()
            .flat_map(|msg| {
                vec![
                    Line::from(Span::styled(
                        format!("  {}", msg.sender),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::raw(format!("  {}", msg.body))),
                    Line::from(""),
                ]
            })
            .collect();

        let messages = Paragraph::new(visible).block(msg_block).wrap(Wrap { trim: false });
        f.render_widget(messages, msg_area);
    }

    // Input box
    let input_focused = app.focus == Focus::Input;
    let input_style = if input_focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dimmed)
    };
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(input_style)
        .title(if input_focused { " > " } else { "" });

    let input_text = Paragraph::new(app.input.as_str()).block(input_block);
    f.render_widget(input_text, input_area);

    // Show cursor in input
    if input_focused {
        f.set_cursor_position((
            input_area.x + 1 + app.cursor_pos as u16,
            input_area.y + 1,
        ));
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let mut spans = Vec::new();

    // Account status dots
    for acct in &app.accounts {
        let color = if acct.syncing {
            theme.status_ok
        } else {
            theme.status_err
        };
        spans.push(Span::styled("● ", Style::default().fg(color)));
        spans.push(Span::raw(format!("{}  ", acct.homeserver)));
    }

    // Separator
    if !app.accounts.is_empty() {
        spans.push(Span::styled("│ ", Style::default().fg(theme.dimmed)));
    }

    // Status message
    spans.push(Span::styled(
        &app.status_msg,
        Style::default().fg(theme.dimmed),
    ));

    // Shortcuts hint (right-aligned would be nice but keep it simple)
    let status = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.status_bg));
    f.render_widget(status, area);
}

fn draw_login_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = centered_rect(50, 14, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Add Account ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fields = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // label
            Constraint::Length(1), // homeserver
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // username
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // password
            Constraint::Length(1), // spacer
            Constraint::Length(1), // error or hint
        ])
        .split(inner);

    let hs_style = field_style(app.login_focus == 0, theme);
    let un_style = field_style(app.login_focus == 1, theme);
    let pw_style = field_style(app.login_focus == 2, theme);

    f.render_widget(
        Paragraph::new("Homeserver:").style(Style::default().fg(theme.text_dim)),
        fields[0],
    );
    f.render_widget(
        Paragraph::new(format!(" {}", app.login_homeserver)).style(hs_style),
        fields[1],
    );

    f.render_widget(
        Paragraph::new("Username:").style(Style::default().fg(theme.text_dim)),
        fields[3],
    );
    f.render_widget(
        Paragraph::new(format!(" {}", app.login_username)).style(un_style),
        fields[4],
    );

    f.render_widget(
        Paragraph::new("Password:").style(Style::default().fg(theme.text_dim)),
        fields[6],
    );
    let masked: String = "●".repeat(app.login_password.len());
    f.render_widget(
        Paragraph::new(format!(" {}", masked)).style(pw_style),
        fields[7],
    );

    // Error or hint
    let hint = if let Some(err) = &app.login_error {
        Paragraph::new(err.as_str()).style(Style::default().fg(theme.status_err))
    } else if app.login_busy {
        Paragraph::new("Logging in...").style(Style::default().fg(theme.status_warn))
    } else {
        Paragraph::new("Tab: next field  Enter: login  Esc: cancel")
            .style(Style::default().fg(theme.dimmed))
    };
    f.render_widget(hint, fields[9]);

    // Cursor position
    if !app.login_busy {
        let (cursor_row, cursor_col) = match app.login_focus {
            0 => (fields[1].y, fields[1].x + 1 + app.login_homeserver.len() as u16),
            1 => (fields[4].y, fields[4].x + 1 + app.login_username.len() as u16),
            2 => (fields[7].y, fields[7].x + 1 + app.login_password.len() as u16),
            _ => (0, 0),
        };
        f.set_cursor_position((cursor_col, cursor_row));
    }
}

fn draw_help_overlay(f: &mut Frame, theme: &Theme) {
    let area = centered_rect(60, 24, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let help_text = vec![
        "",
        "  Navigation:",
        "    Tab/Shift+Tab    Cycle panels",
        "    Arrow keys       Navigate within panel",
        "    Enter            Select room / send message",
        "    Esc              Back",
        "",
        "  Global:",
        "    Ctrl+K           Quick room switcher",
        "    Ctrl+Q           Quit",
        "    a                Add account",
        "    s                Settings / themes",
        "    ?                Toggle this help",
        "",
        "  Rooms:",
        "    f                Toggle favorite",
        "    Shift+Up/Down    Reorder favorites",
        "",
        "  Chat:",
        "    Enter            Start typing",
        "    Up/Down          Scroll messages",
    ];

    let text: Vec<Line> = help_text.iter().map(|&s| Line::from(s)).collect();
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_switcher_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = centered_rect(50, 12, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Jump to room ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    // Search input
    f.render_widget(
        Paragraph::new(format!(" > {}", app.switcher_query)),
        layout[0],
    );
    f.set_cursor_position((
        layout[0].x + 4 + app.switcher_query.len() as u16,
        layout[0].y,
    ));

    // Separator
    f.render_widget(
        Paragraph::new("─".repeat(layout[1].width as usize))
            .style(Style::default().fg(theme.dimmed)),
        layout[1],
    );

    // Filtered results
    let filtered = app.filtered_rooms();
    let items: Vec<ListItem> = filtered
        .iter()
        .take(layout[2].height as usize)
        .enumerate()
        .map(|(i, room)| {
            let prefix = if room.is_dm { " @" } else { " #" };
            let style = if i == app.switcher_selected {
                Style::default().fg(theme.accent).bg(theme.highlight_bg)
            } else {
                Style::default()
            };
            // Pad account_id to right
            let max_name = (layout[2].width as usize).saturating_sub(room.account_id.len() + 4);
            let name = if room.name.len() > max_name {
                format!("{}…", &room.name[..max_name.saturating_sub(1)])
            } else {
                format!("{:width$}", room.name, width = max_name)
            };
            ListItem::new(format!("{}{} {}", prefix, name, room.account_id)).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, layout[2]);
}

fn draw_settings_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;

    // Dynamic height based on expanded sub-menus
    let mut content_lines: u16 = 6; // top_pad + Accounts + Theme + Sort + bottom_pad + hint
    if app.settings_accounts_open {
        content_lines += 1 + app.accounts.len() as u16; // Add Account + each account
        if app.settings_account_action_open {
            content_lines += 2; // Reconnect + Remove
        }
    }
    if app.settings_theme_open {
        content_lines += builtin_themes().len() as u16;
    }
    if app.settings_sort_open {
        content_lines += RoomSortMode::ALL.len() as u16;
    }
    let height = content_lines + 2; // +2 for borders

    let area = centered_rect(60, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Top padding
    lines.push(Line::from(""));

    // --- Accounts item ---
    let at_top = !app.settings_accounts_open && !app.settings_theme_open && !app.settings_sort_open;
    let sel0 = at_top && app.settings_selected == 0;
    let acct_count = app.accounts.len();
    let (prefix0, style0) = if sel0 {
        (
            "  > ",
            Style::default()
                .fg(theme.text)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
    } else if app.settings_accounts_open {
        (
            "  \u{25b8} ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("    ", Style::default().fg(theme.text_dim))
    };
    lines.push(Line::from(Span::styled(
        format!("{}Accounts ({})", prefix0, acct_count),
        style0,
    )));

    // --- Accounts sub-menu ---
    if app.settings_accounts_open {
        // Add Account button
        let is_add_sel =
            !app.settings_account_action_open && app.settings_accounts_selected == 0;
        let add_prefix = if is_add_sel { "      > " } else { "        " };
        let add_style = if is_add_sel {
            Style::default()
                .fg(theme.text)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        lines.push(Line::from(Span::styled(
            format!("{}Add Account", add_prefix),
            add_style,
        )));

        // Active accounts
        for (i, acct) in app.accounts.iter().enumerate() {
            let acct_sel_idx = i + 1;
            let is_sel = !app.settings_account_action_open
                && app.settings_accounts_selected == acct_sel_idx;
            let is_action_target = app.settings_account_action_open
                && app.settings_accounts_selected == acct_sel_idx;
            let dot = if acct.syncing { "\u{25cf}" } else { "\u{25cb}" };
            let prefix = if is_sel { "      > " } else { "        " };
            let style = if is_sel || is_action_target {
                Style::default()
                    .fg(theme.text)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_dim)
            };
            lines.push(Line::from(Span::styled(
                format!("{}{} {}", prefix, dot, acct.user_id),
                style,
            )));

            // Action menu for this account
            if is_action_target {
                let actions = ["Reconnect", "Remove Account"];
                for (j, action) in actions.iter().enumerate() {
                    let is_action_sel = app.settings_account_action_selected == j;
                    let action_prefix = if is_action_sel {
                        "          > "
                    } else {
                        "            "
                    };
                    let action_style = if is_action_sel {
                        Style::default()
                            .fg(if j == 1 { theme.status_err } else { theme.text })
                            .bg(theme.highlight_bg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(if j == 1 { theme.status_err } else { theme.text_dim })
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{}{}", action_prefix, action),
                        action_style,
                    )));
                }
            }
        }
    }

    // --- Theme item ---
    let sel1 = at_top && app.settings_selected == 1;
    let (prefix1, style1) = if sel1 {
        (
            "  > ",
            Style::default()
                .fg(theme.text)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
    } else if app.settings_theme_open {
        (
            "  \u{25b8} ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("    ", Style::default().fg(theme.text_dim))
    };
    lines.push(Line::from(Span::styled(
        format!("{}Theme: {}", prefix1, app.theme.name),
        style1,
    )));

    // --- Theme sub-list ---
    if app.settings_theme_open {
        let themes = builtin_themes();
        for (i, t) in themes.iter().enumerate() {
            let is_active = t.name == app.theme.name;
            let is_sel = i == app.settings_theme_selected;
            let prefix = if is_sel { "      > " } else { "        " };
            let suffix = if is_active { " \u{2713}" } else { "" };
            let style = if is_sel {
                Style::default()
                    .fg(theme.text)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.text_dim)
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}{}", prefix, t.name, suffix),
                style,
            )));
        }
    }

    // --- Sort item ---
    let sel2 = at_top && app.settings_selected == 2;
    let (prefix2, style2) = if sel2 {
        (
            "  > ",
            Style::default()
                .fg(theme.text)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
    } else if app.settings_sort_open {
        (
            "  \u{25b8} ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("    ", Style::default().fg(theme.text_dim))
    };
    lines.push(Line::from(Span::styled(
        format!("{}Sort: {}", prefix2, app.room_sort.label()),
        style2,
    )));

    // --- Sort sub-list ---
    if app.settings_sort_open {
        for (i, mode) in RoomSortMode::ALL.iter().enumerate() {
            let is_active = *mode == app.room_sort;
            let is_sel = i == app.settings_sort_selected;
            let prefix = if is_sel { "      > " } else { "        " };
            let suffix = if is_active { " \u{2713}" } else { "" };
            let style = if is_sel {
                Style::default()
                    .fg(theme.text)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.text_dim)
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}{}", prefix, mode.label(), suffix),
                style,
            )));
        }
    }

    // Bottom padding
    lines.push(Line::from(""));

    // Hint
    let hint_text = if app.settings_account_action_open || app.settings_theme_open || app.settings_sort_open {
        "  \u{2191}/\u{2193} select   Enter apply   Esc back"
    } else {
        "  \u{2191}/\u{2193} select   Enter open   Esc back"
    };
    lines.push(Line::from(Span::styled(
        hint_text,
        Style::default().fg(theme.dimmed),
    )));

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn field_style(focused: bool, theme: &Theme) -> Style {
    if focused {
        Style::default().fg(theme.text).bg(theme.highlight_bg)
    } else {
        Style::default().fg(theme.text_dim)
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = (area.width * percent_x / 100).min(area.width);
    let popup_height = height.min(area.height);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    Rect::new(area.x + x, area.y + y, popup_width, popup_height)
}
