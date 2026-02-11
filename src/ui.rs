use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, DisplayMessage, Focus, Overlay};

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
        Overlay::Help => draw_help_overlay(f),
        Overlay::RoomSwitcher => draw_switcher_overlay(f, app),
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
    let focused = app.focus == Focus::Accounts;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
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
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(" {} {}", marker, label)).style(style)
        })
        .collect();

    items.push(ListItem::new("").style(Style::default()));
    items.push(
        ListItem::new(" [a] Add")
            .style(Style::default().fg(Color::DarkGray)),
    );

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_rooms_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Rooms;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Rooms ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let items: Vec<ListItem> = app
        .all_rooms
        .iter()
        .enumerate()
        .map(|(i, room)| {
            let prefix = if room.is_dm { "@" } else { "#" };
            let unread = if room.unread > 0 {
                format!(" ({})", room.unread)
            } else {
                String::new()
            };

            let is_active = Some(&room.id) == app.active_room.as_ref();
            let is_selected = i == app.selected_room;

            let style = if is_active {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_selected && focused {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else if room.unread > 0 {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            // Truncate name to fit
            let max_name = (area.width as usize).saturating_sub(6);
            let name = if room.name.len() > max_name {
                format!("{}…", &room.name[..max_name.saturating_sub(1)])
            } else {
                room.name.clone()
            };

            ListItem::new(format!(" {}{}{}", prefix, name, unread)).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new(" No rooms yet\n\n Press 'a' to\n add an account")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(empty, area);
    } else {
        let list = List::new(items).block(block);
        f.render_widget(list, area);
    }
}

fn draw_chat_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Chat || app.focus == Focus::Input;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
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
        let welcome = Paragraph::new("\n  Select a room to start chatting\n\n  Ctrl+K  quick room switcher\n  a       add account\n  ?       help")
            .style(Style::default().fg(Color::DarkGray))
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
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
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
    let mut spans = Vec::new();

    // Account status dots
    for acct in &app.accounts {
        let color = if acct.syncing {
            Color::Green
        } else {
            Color::Red
        };
        spans.push(Span::styled("● ", Style::default().fg(color)));
        spans.push(Span::raw(format!("{}  ", acct.homeserver)));
    }

    // Separator
    if !app.accounts.is_empty() {
        spans.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
    }

    // Status message
    spans.push(Span::styled(
        &app.status_msg,
        Style::default().fg(Color::DarkGray),
    ));

    // Shortcuts hint (right-aligned would be nice but keep it simple)
    let status = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Black));
    f.render_widget(status, area);
}

fn draw_login_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 14, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Add Account ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

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

    let hs_style = field_style(app.login_focus == 0);
    let un_style = field_style(app.login_focus == 1);
    let pw_style = field_style(app.login_focus == 2);

    f.render_widget(
        Paragraph::new("Homeserver:").style(Style::default().fg(Color::Gray)),
        fields[0],
    );
    f.render_widget(
        Paragraph::new(format!(" {}", app.login_homeserver)).style(hs_style),
        fields[1],
    );

    f.render_widget(
        Paragraph::new("Username:").style(Style::default().fg(Color::Gray)),
        fields[3],
    );
    f.render_widget(
        Paragraph::new(format!(" {}", app.login_username)).style(un_style),
        fields[4],
    );

    f.render_widget(
        Paragraph::new("Password:").style(Style::default().fg(Color::Gray)),
        fields[6],
    );
    let masked: String = "●".repeat(app.login_password.len());
    f.render_widget(
        Paragraph::new(format!(" {}", masked)).style(pw_style),
        fields[7],
    );

    // Error or hint
    let hint = if let Some(err) = &app.login_error {
        Paragraph::new(err.as_str()).style(Style::default().fg(Color::Red))
    } else if app.login_busy {
        Paragraph::new("Logging in...").style(Style::default().fg(Color::Yellow))
    } else {
        Paragraph::new("Tab: next field  Enter: login  Esc: cancel")
            .style(Style::default().fg(Color::DarkGray))
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

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

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
        "    ?                Toggle this help",
        "",
        "  Chat:",
        "    Enter            Start typing",
        "    Up/Down          Scroll messages",
        "",
        "  Press ? or Esc to close",
    ];

    let text: Vec<Line> = help_text.iter().map(|&s| Line::from(s)).collect();
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_switcher_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 12, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Jump to room ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

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
            .style(Style::default().fg(Color::DarkGray)),
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
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
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

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = (area.width * percent_x / 100).min(area.width);
    let popup_height = height.min(area.height);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    Rect::new(area.x + x, area.y + y, popup_width, popup_height)
}
