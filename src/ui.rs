use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use ratatui_image::StatefulImage;

use crate::app::{App, FileKind, Focus, MessageContent, Overlay, RoomSortMode, SasOverlayState};
use matrix_sdk::ruma::events::room::MediaSource;

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
        Overlay::Help => draw_help_overlay(f, app),
        Overlay::RoomSwitcher => draw_switcher_overlay(f, app),
        Overlay::Settings => draw_settings_overlay(f, app),
        Overlay::ProfileEditor => draw_profile_overlay(f, app),
        Overlay::RoomCreator => draw_creator_overlay(f, app),
        Overlay::RoomEditor => draw_editor_overlay(f, app),
        Overlay::Recovery => draw_recovery_overlay(f, app),
        Overlay::MessageAction => draw_message_action_overlay(f, app),
        Overlay::SasVerify => draw_sas_verify_overlay(f, app),
        Overlay::EmojiPicker => draw_emoji_picker_overlay(f, app),
        Overlay::RoomInfo => draw_room_info_overlay(f, app),
        Overlay::FileConfirm => draw_file_confirm_overlay(f, app),
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

/// Pre-wrap text into Lines, each prefixed with `indent`, fitting within `width` columns.
/// Unlike Paragraph::wrap, continuation lines keep the same indent as line 1.
fn wrap_with_indent<'a>(text: &str, indent: &str, width: usize, style: Style) -> Vec<Line<'a>> {
    let indent_w = indent.chars().count();
    let content_w = width.saturating_sub(indent_w).max(1);
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![Line::from(Span::styled(indent.to_string(), style))];
    }
    chars
        .chunks(content_w)
        .map(|chunk| {
            Line::from(Span::styled(
                format!("{}{}", indent, chunk.iter().collect::<String>()),
                style,
            ))
        })
        .collect()
}

/// Calculate how many visual lines text occupies when wrapped with indent
fn wrapped_height_indented(text_chars: usize, indent_chars: usize, width: usize) -> usize {
    let content_w = width.saturating_sub(indent_chars).max(1);
    if text_chars == 0 { 1 } else { (text_chars + content_w - 1) / content_w }
}

/// Build an HTTP download URL from an MXC media source (unencrypted only)
fn media_download_url(source: &MediaSource, homeserver: &str) -> Option<String> {
    match source {
        MediaSource::Plain(mxc_uri) => {
            let stripped = mxc_uri.as_str().strip_prefix("mxc://")?;
            let hs = if homeserver.starts_with("http") {
                homeserver.trim_end_matches('/')
            } else {
                return Some(format!(
                    "https://{}/_matrix/media/v3/download/{}",
                    homeserver, stripped
                ));
            };
            Some(format!("{}/_matrix/media/v3/download/{}", hs, stripped))
        }
        MediaSource::Encrypted(_) => None,
    }
}

/// Inject OSC 8 terminal hyperlink escape sequences into buffer cells
fn inject_osc8_link(buf: &mut Buffer, x: u16, y: u16, len: u16, url: &str) {
    if len == 0 {
        return;
    }
    // Prepend OSC 8 open to the first cell
    if let Some(cell) = buf.cell_mut(ratatui::layout::Position { x, y }) {
        let sym = cell.symbol().to_string();
        cell.set_symbol(&format!("\x1b]8;;{}\x07{}", url, sym));
    }
    // Append OSC 8 close to the last cell
    let end_x = x + len.saturating_sub(1);
    if let Some(cell) = buf.cell_mut(ratatui::layout::Position { x: end_x, y }) {
        let sym = cell.symbol().to_string();
        cell.set_symbol(&format!("{}\x1b]8;;\x07", sym));
    }
}

fn draw_chat_panel(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Chat || app.focus == Focus::Input;
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dimmed)
    };

    let title = if app.downloading_keys {
        " Downloading room keys... ".to_string()
    } else if let Some(room_id) = &app.active_room {
        if let Some(room) = app.all_rooms.iter().find(|r| &r.id == room_id) {
            format!(" {} · {} ", room.name, room.account_id)
        } else {
            " Chat ".to_string()
        }
    } else {
        " Chat ".to_string()
    };

    let title_style = if app.downloading_keys {
        Style::default().fg(theme.status_ok).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    // Calculate input box height based on wrapped text
    let inner_width = (area.width as usize).saturating_sub(2); // borders
    let input_lines = if inner_width == 0 {
        1
    } else {
        let len = app.input.len();
        if len == 0 { 1 } else { (len + inner_width - 1) / inner_width }
    };
    let max_input_lines = ((area.height as usize).saturating_sub(5)) / 2; // cap at half of chat area
    let clamped_lines = input_lines.clamp(1, max_input_lines.max(1));
    let input_height = (clamped_lines as u16) + 2; // +2 for borders

    // Typing indicator height
    let typing_height: u16 = if !app.typing_users.is_empty() { 1 } else { 0 };

    // Split chat area: messages + typing + input
    let chat_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(typing_height),
            Constraint::Length(input_height),
        ])
        .split(area);

    let msg_area = chat_layout[0];
    let typing_area = chat_layout[1];
    let input_area = chat_layout[2];

    // Messages
    let msg_block = Block::default()
        .title(Span::styled(title, title_style))
        .borders(Borders::ALL)
        .border_style(if app.downloading_keys { Style::default().fg(theme.status_ok) } else { border_style });

    if app.messages.is_empty() && app.active_room.is_none() {
        let welcome = Paragraph::new("\n  Select a room to start chatting\n\n  Ctrl+K  quick room switcher\n  a       add account\n  n       new room\n  e       edit active room\n  s       settings\n  ?       help")
            .style(Style::default().fg(theme.dimmed))
            .block(msg_block);
        f.render_widget(welcome, msg_area);
    } else {
        let msg_height = msg_area.height.saturating_sub(2) as usize;
        let inner_width = msg_area.width.saturating_sub(2) as usize; // borders

        let end = if app.messages.len() > app.scroll_offset {
            app.messages.len() - app.scroll_offset
        } else {
            app.messages.len()
        };

        // Measure messages from the bottom up to find how many actually fit,
        // accounting for line wrapping with consistent indent
        let mut used_height = 0usize;
        let mut start = end;
        for i in (0..end).rev() {
            let msg = &app.messages[i];
            let is_reply = msg.reply_to_sender.is_some();
            let indent = if is_reply { "    " } else { "  " };
            let indent_w = indent.chars().count();
            let mut msg_h = wrapped_height_indented(msg.sender.chars().count(), indent_w, inner_width);
            match &msg.content {
                MessageContent::Image { protocol, .. } => {
                    if protocol.is_some() {
                        msg_h += 8; // image display height
                    } else {
                        msg_h += 1; // loading/fallback text
                    }
                }
                MessageContent::File { body, media_type, .. } => {
                    let prefix = match media_type {
                        FileKind::File  => "[file: ",
                        FileKind::Video => "[video: ",
                        FileKind::Audio => "[audio: ",
                    };
                    let content = format!("{}{}]", prefix, body);
                    msg_h += wrapped_height_indented(content.chars().count(), indent_w, inner_width);
                }
                MessageContent::Text(_) => {
                    let body_str = msg.body_text();
                    msg_h += wrapped_height_indented(body_str.chars().count(), indent_w, inner_width);
                }
            }
            // Reply context line (may wrap)
            if is_reply {
                let reply_content = format!("\u{2514} {}: {}",
                    msg.reply_to_sender.as_deref().unwrap_or(""),
                    msg.reply_to_body.as_deref().unwrap_or(""));
                msg_h += wrapped_height_indented(reply_content.chars().count(), 2, inner_width);
            }
            // Reaction line
            if !msg.reactions.is_empty() {
                msg_h += 1;
            }
            // Unread separator
            if app.first_unread_index == Some(i) {
                msg_h += 1;
            }
            // Separator line between messages (not after the last one)
            if i + 1 < end {
                msg_h += 1;
            }
            if used_height + msg_h > msg_height {
                break;
            }
            used_height += msg_h;
            start = i;
        }
        let msgs_per_page = end - start;
        app.chat_viewport_msgs.set(msgs_per_page.max(1));

        let visible_msgs = &app.messages[start..end];
        let msg_count = visible_msgs.len();

        // Track image positions: (line_offset, msg_index_in_visible)
        let mut image_positions: Vec<(usize, usize)> = Vec::new();
        // Track link positions for OSC 8: (line_offset, source, text_len)
        let mut link_positions: Vec<(usize, MediaSource, usize)> = Vec::new();
        let mut visible: Vec<Line> = Vec::new();

        for (i, msg) in visible_msgs.iter().enumerate() {
            let msg_idx = start + i;
            let is_selected = app.selected_message == Some(msg_idx);
            let sender_style = if is_selected {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            };
            let body_style = if is_selected {
                Style::default()
                    .fg(theme.text)
                    .bg(theme.highlight_bg)
            } else {
                Style::default()
            };

            // Unread separator
            if app.first_unread_index == Some(msg_idx) {
                let sep_width = inner_width.saturating_sub(4);
                let dashes = "\u{2500}".repeat(sep_width / 2);
                let sep_text = format!("{}  new  {}", dashes, dashes);
                visible.push(Line::from(Span::styled(
                    sep_text,
                    Style::default().fg(theme.status_warn),
                )));
            }

            // Reply context line + indented sender/body for replies
            let is_reply = msg.reply_to_sender.is_some();
            if let (Some(reply_sender), Some(reply_body)) =
                (&msg.reply_to_sender, &msg.reply_to_body)
            {
                let reply_content = format!("\u{2514} {}: {}", reply_sender, reply_body);
                let reply_style = Style::default()
                    .fg(theme.text_dim)
                    .add_modifier(Modifier::ITALIC);
                visible.extend(wrap_with_indent(&reply_content, "  ", inner_width, reply_style));
            }

            let indent = if is_reply { "    " } else { "  " };
            visible.extend(wrap_with_indent(&msg.sender, indent, inner_width, sender_style));

            match &msg.content {
                MessageContent::Image { body, loading, protocol, source, .. } => {
                    if protocol.is_some() {
                        // Record the line offset where the image should render
                        image_positions.push((visible.len(), msg_idx));
                        // Reserve 8 lines for the image
                        for _ in 0..8 {
                            visible.push(Line::from(""));
                        }
                    } else if *loading {
                        let load_text = format!("[loading {}...]", body);
                        let load_style = Style::default()
                            .fg(theme.text_dim)
                            .add_modifier(Modifier::ITALIC);
                        visible.extend(wrap_with_indent(&load_text, indent, inner_width, load_style));
                    } else {
                        let display_content = format!("[image: {}]", body);
                        let text_len = indent.len() + display_content.len();
                        link_positions.push((visible.len(), source.clone(), text_len));
                        let link_style = Style::default().fg(theme.text_dim)
                            .add_modifier(Modifier::UNDERLINED);
                        visible.extend(wrap_with_indent(&display_content, indent, inner_width, link_style));
                    }
                }
                MessageContent::File { body, source, media_type } => {
                    let prefix = match media_type {
                        FileKind::File  => "[file: ",
                        FileKind::Video => "[video: ",
                        FileKind::Audio => "[audio: ",
                    };
                    let display_content = format!("{}{}]", prefix, body);
                    let text_len = indent.len() + display_content.len();
                    link_positions.push((visible.len(), source.clone(), text_len));
                    let link_style = Style::default().fg(theme.text_dim)
                        .add_modifier(Modifier::UNDERLINED);
                    visible.extend(wrap_with_indent(&display_content, indent, inner_width, link_style));
                }
                MessageContent::Text(_) => {
                    let body_str = msg.body_text();
                    visible.extend(wrap_with_indent(body_str, indent, inner_width, body_style));
                }
            }

            // Reaction line
            if !msg.reactions.is_empty() {
                let reaction_text: String = msg
                    .reactions
                    .iter()
                    .map(|(emoji, count)| {
                        if *count > 1 {
                            format!("{} {} ", emoji, count)
                        } else {
                            format!("{} ", emoji)
                        }
                    })
                    .collect();
                visible.push(Line::from(Span::styled(
                    format!("  {}", reaction_text.trim_end()),
                    Style::default().fg(theme.text_dim),
                )));
            }

            // Add separator after every message except the last
            if i + 1 < msg_count {
                visible.push(Line::from(""));
            }
        }

        // Bottom-align: pad top with empty lines so messages anchor to the bottom
        let top_padding = if used_height < msg_height {
            let padding = msg_height - used_height;
            let mut padded = vec![Line::from(""); padding];
            let pad_count = padded.len();
            padded.append(&mut visible);
            visible = padded;
            pad_count
        } else {
            0
        };

        let messages = Paragraph::new(visible).block(msg_block).wrap(Wrap { trim: false });
        f.render_widget(messages, msg_area);

        // Render StatefulImage widgets as overlays at tracked positions
        let inner_area = Rect::new(
            msg_area.x + 1,
            msg_area.y + 1,
            msg_area.width.saturating_sub(2),
            msg_area.height.saturating_sub(2),
        );
        for (line_offset, msg_idx) in &image_positions {
            if let Some(msg) = app.messages.get(*msg_idx) {
                if let MessageContent::Image { protocol: Some(ref proto), .. } = msg.content {
                    let y_offset = (*line_offset + top_padding) as u16;
                    if y_offset < inner_area.height {
                        let img_height = 8u16.min(inner_area.height.saturating_sub(y_offset));
                        let img_area = Rect::new(
                            inner_area.x + 2, // indent
                            inner_area.y + y_offset,
                            inner_area.width.saturating_sub(4),
                            img_height,
                        );
                        if let Ok(mut guard) = proto.lock() {
                            let image_widget = StatefulImage::default();
                            f.render_stateful_widget(image_widget, img_area, &mut *guard);
                        }
                    }
                }
            }
        }

        // Inject OSC 8 hyperlinks for media messages (unencrypted only)
        let homeserver = app.active_account_id.as_ref()
            .and_then(|id| app.accounts.iter().find(|a| &a.user_id == id))
            .map(|a| a.homeserver.as_str())
            .unwrap_or("");

        for (line_offset, source, text_len) in &link_positions {
            if let Some(url) = media_download_url(source, homeserver) {
                let y = inner_area.y + (*line_offset + top_padding) as u16;
                if y < inner_area.bottom() {
                    inject_osc8_link(
                        f.buffer_mut(),
                        inner_area.x,
                        y,
                        (*text_len as u16).min(inner_area.width),
                        &url,
                    );
                }
            }
        }
    }

    // Typing indicator
    if !app.typing_users.is_empty() {
        let typing_text = if app.typing_users.len() == 1 {
            format!("  {} is typing...", app.typing_users[0])
        } else {
            format!("  {} are typing...", app.typing_users.join(", "))
        };
        let typing = Paragraph::new(Span::styled(
            typing_text,
            Style::default()
                .fg(theme.text_dim)
                .add_modifier(Modifier::ITALIC),
        ));
        f.render_widget(typing, typing_area);
    }

    // Input box
    let input_focused = app.focus == Focus::Input;
    let input_style = if input_focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dimmed)
    };
    let input_title = if let Some((_, ref sender, _)) = app.replying_to {
        let short_name = sender.split(':').next().unwrap_or(sender);
        format!(" Reply to {} (Esc cancel) ", short_name)
    } else if input_focused {
        " > ".to_string()
    } else {
        String::new()
    };
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(input_style)
        .title(input_title);

    let input_text = Paragraph::new(app.input.as_str())
        .block(input_block)
        .wrap(Wrap { trim: false });
    f.render_widget(input_text, input_area);

    // Show cursor in input (accounting for wrap)
    if input_focused {
        let iw = inner_width.max(1);
        let cursor_row = app.cursor_pos / iw;
        let cursor_col = app.cursor_pos % iw;
        f.set_cursor_position((
            input_area.x + 1 + cursor_col as u16,
            input_area.y + 1 + cursor_row as u16,
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
    let base_width = (f.area().width * 50 / 100).min(f.area().width);
    let err_lines: u16 = if let Some(err) = &app.login_error {
        let avail = base_width.saturating_sub(4) as usize;
        if avail == 0 { 1 } else { ((err.len() / avail) + 1).min(3) as u16 }
    } else {
        1
    };
    let height = (11 + err_lines).min(f.area().height);

    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Add Account ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fields = Layout::default()
        .direction(Direction::Vertical)
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
            Constraint::Min(1),   // error or hint (wrapping)
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
    let masked: String = "\u{25cf}".repeat(app.login_password.len());
    f.render_widget(
        Paragraph::new(format!(" {}", masked)).style(pw_style),
        fields[7],
    );

    // Error or hint
    let hint = if let Some(err) = &app.login_error {
        Paragraph::new(err.as_str())
            .style(Style::default().fg(theme.status_err))
            .wrap(Wrap { trim: false })
    } else if app.login_busy {
        Paragraph::new("Logging in...")
            .style(Style::default().fg(theme.status_warn))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new("Tab: next  Enter: login  Esc: cancel")
            .style(Style::default().fg(theme.dimmed))
            .wrap(Wrap { trim: false })
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

fn draw_help_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let term = f.area();

    let help_text = vec![
        "",
        "  Navigation:",
        "    Tab/Shift+Tab    Cycle panels",
        "    Arrow keys       Navigate within panel",
        "    Enter            Select room / send message",
        "    Esc              Back / deselect",
        "",
        "  Global:",
        "    Ctrl+K           Quick room switcher",
        "    Ctrl+Q           Quit",
        "    a                Add account",
        "    s                Settings / themes",
        "    n                New room",
        "    e                Edit active room",
        "    ?                Toggle this help",
        "",
        "  Rooms:",
        "    f                Toggle favorite",
        "    Shift+Up/Down    Reorder favorites",
        "",
        "  Chat:",
        "    Up/Down          Select / scroll messages",
        "    Enter            Message actions (edit/delete)",
        "    r                Reply to selected message",
        "    e                React to selected message",
        "    Ctrl+I           Room info panel",
        "    Tab              Focus input box",
        "    Esc              Deselect / back to rooms",
        "    Home/End         Jump to oldest / newest",
    ];

    let content_height = help_text.len() as u16;
    let height = (content_height + 2).min(term.height); // +2 for borders

    let area = centered_rect(60, height, term);
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help (\u{2191}/\u{2193} scroll) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible_height = inner.height as usize;
    let max_scroll = (help_text.len()).saturating_sub(visible_height);
    let scroll = app.help_scroll.min(max_scroll);

    let visible_lines: Vec<Line> = help_text
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|&s| Line::from(s))
        .collect();

    let paragraph = Paragraph::new(visible_lines);
    f.render_widget(paragraph, inner);
}

fn draw_switcher_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let filtered_count = app.filtered_rooms().len() as u16;
    let result_rows = filtered_count.clamp(1, 8);
    let height = (result_rows + 4).min(f.area().height); // +2 search+separator, +2 borders
    let area = centered_rect(50, height, f.area());
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
    let mut content_lines: u16 = 7; // top_pad + Accounts + Theme + Sort + Clear Cache + bottom_pad + hint
    if app.settings_accounts_open {
        content_lines += 1 + app.accounts.len() as u16; // Add Account + each account
        if app.settings_account_action_open {
            content_lines += 4; // Reconnect + Remove + Edit Profile + Verify Session
            if app.settings_verify_open {
                content_lines += 2; // Recovery Key + Another Device
            }
        }
    }
    if app.settings_theme_open {
        content_lines += builtin_themes().len() as u16;
    }
    if app.settings_sort_open {
        content_lines += RoomSortMode::ALL.len() as u16;
    }
    let height = (content_lines + 2).min(f.area().height); // +2 for borders, cap to terminal

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
                let actions = ["Reconnect", "Remove Account", "Edit Profile", "Verify Session"];
                for (j, action) in actions.iter().enumerate() {
                    let is_action_sel = !app.settings_verify_open
                        && app.settings_account_action_selected == j;
                    let is_verify_parent = app.settings_verify_open
                        && app.settings_account_action_selected == 3
                        && j == 3;
                    let action_prefix = if is_action_sel {
                        "          > "
                    } else if is_verify_parent {
                        "          \u{25b8} "
                    } else {
                        "            "
                    };
                    let action_style = if is_action_sel || is_verify_parent {
                        Style::default()
                            .fg(if j == 1 { theme.status_err } else { theme.text })
                            .bg(if is_verify_parent { Color::Reset } else { theme.highlight_bg })
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(if j == 1 { theme.status_err } else { theme.text_dim })
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{}{}", action_prefix, action),
                        action_style,
                    )));

                    // Verify Session sub-menu
                    if j == 3 && app.settings_verify_open {
                        let verify_actions = ["Recovery Key", "Another Device"];
                        for (k, vaction) in verify_actions.iter().enumerate() {
                            let is_vsel = app.settings_verify_selected == k;
                            let vprefix = if is_vsel {
                                "              > "
                            } else {
                                "                "
                            };
                            let vstyle = if is_vsel {
                                Style::default()
                                    .fg(theme.text)
                                    .bg(theme.highlight_bg)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme.text_dim)
                            };
                            lines.push(Line::from(Span::styled(
                                format!("{}{}", vprefix, vaction),
                                vstyle,
                            )));
                        }
                    }
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

    // --- Clear Cache item ---
    let sel3 = at_top && app.settings_selected == 3;
    let (prefix3, style3) = if sel3 {
        (
            "  > ",
            Style::default()
                .fg(theme.status_err)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("    ", Style::default().fg(theme.status_err))
    };
    lines.push(Line::from(Span::styled(
        format!("{}Clear Cache", prefix3),
        style3,
    )));

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

fn draw_profile_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let err_lines: u16 = if app.profile_error.is_some() { 2 } else { 1 };
    let height = (15 + err_lines).min(f.area().height);
    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Edit Profile ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fields = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // user id
            Constraint::Length(1), // current name
            Constraint::Length(1), // current avatar
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // display name field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // avatar url field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // avatar path field
            Constraint::Length(1), // spacer
            Constraint::Min(1),   // error/hint (wrapping)
        ])
        .split(inner);

    let acct_label = if app.profile_account_idx < app.accounts.len() {
        app.accounts[app.profile_account_idx].user_id.as_str()
    } else {
        ""
    };
    f.render_widget(
        Paragraph::new(format!("  {}", acct_label)).style(Style::default().fg(theme.accent)),
        fields[0],
    );
    f.render_widget(
        Paragraph::new(format!("  Name: {}", app.profile_current_name))
            .style(Style::default().fg(theme.text_dim)),
        fields[1],
    );
    let max_avatar = (inner.width as usize).saturating_sub(12); // "  Avatar: " + margin
    let avatar_display = if app.profile_current_avatar.len() > max_avatar && max_avatar > 3 {
        format!("{}...", &app.profile_current_avatar[..max_avatar.saturating_sub(3)])
    } else {
        app.profile_current_avatar.clone()
    };
    f.render_widget(
        Paragraph::new(format!("  Avatar: {}", avatar_display))
            .style(Style::default().fg(theme.text_dim)),
        fields[2],
    );

    let s0 = field_style(app.profile_focus == 0, theme);
    let s1 = field_style(app.profile_focus == 1, theme);
    let s2 = field_style(app.profile_focus == 2, theme);

    f.render_widget(
        Paragraph::new("  Display Name:").style(Style::default().fg(theme.text_dim)),
        fields[4],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.profile_display_name)).style(s0),
        fields[5],
    );
    f.render_widget(
        Paragraph::new("  Avatar MXC URL:").style(Style::default().fg(theme.text_dim)),
        fields[7],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.profile_avatar_url)).style(s1),
        fields[8],
    );
    f.render_widget(
        Paragraph::new("  Upload Avatar (file path):").style(Style::default().fg(theme.text_dim)),
        fields[10],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.profile_avatar_path)).style(s2),
        fields[11],
    );

    let hint = if let Some(err) = &app.profile_error {
        Paragraph::new(format!("  {}", err))
            .style(Style::default().fg(theme.status_err))
            .wrap(Wrap { trim: false })
    } else if app.profile_busy {
        Paragraph::new("  Working...")
            .style(Style::default().fg(theme.status_warn))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new("  Tab: next  Enter: apply  Esc: back")
            .style(Style::default().fg(theme.dimmed))
            .wrap(Wrap { trim: false })
    };
    f.render_widget(hint, fields[13]);

    if !app.profile_busy {
        let (row, col) = match app.profile_focus {
            0 => (fields[5].y, fields[5].x + 2 + app.profile_display_name.len() as u16),
            1 => (fields[8].y, fields[8].x + 2 + app.profile_avatar_url.len() as u16),
            2 => (fields[11].y, fields[11].x + 2 + app.profile_avatar_path.len() as u16),
            _ => (0, 0),
        };
        f.set_cursor_position((col, row));
    }
}

fn draw_creator_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let err_lines: u16 = if app.creator_error.is_some() { 2 } else { 1 };
    let height = (17 + err_lines).min(f.area().height);
    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Room ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fields = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // account selector
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // name field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // topic field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // visibility
            Constraint::Length(1), // encryption
            Constraint::Length(1), // federated
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // invite field
            Constraint::Length(1), // spacer
            Constraint::Min(1),   // error/hint (wrapping)
        ])
        .split(inner);

    // Account selector (focus 0)
    let acct_label = if app.accounts.is_empty() {
        "(no accounts)".to_string()
    } else {
        let acct = &app.accounts[app.creator_account_idx.min(app.accounts.len() - 1)];
        acct.user_id.clone()
    };
    let acct_style = if app.creator_focus == 0 {
        Style::default().fg(theme.text).bg(theme.highlight_bg)
    } else {
        Style::default().fg(theme.text_dim)
    };
    let arrows = if app.accounts.len() > 1 { "◄ ► " } else { "" };
    f.render_widget(
        Paragraph::new(format!("  Account:  {}[{}]", arrows, acct_label)).style(acct_style),
        fields[0],
    );

    let s1 = field_style(app.creator_focus == 1, theme);
    let s2 = field_style(app.creator_focus == 2, theme);
    let s6 = field_style(app.creator_focus == 6, theme);

    f.render_widget(
        Paragraph::new("  Name:").style(Style::default().fg(theme.text_dim)),
        fields[2],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.creator_name)).style(s1),
        fields[3],
    );
    f.render_widget(
        Paragraph::new("  Topic:").style(Style::default().fg(theme.text_dim)),
        fields[5],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.creator_topic)).style(s2),
        fields[6],
    );

    let vis_label = if app.creator_visibility == 1 { "Public" } else { "Private" };
    let vis_style = if app.creator_focus == 3 {
        Style::default().fg(theme.text).bg(theme.highlight_bg)
    } else {
        Style::default().fg(theme.text_dim)
    };
    f.render_widget(
        Paragraph::new(format!("  Visibility:   [{}]", vis_label)).style(vis_style),
        fields[8],
    );

    let e2ee_label = if app.creator_e2ee { "On" } else { "Off" };
    let e2ee_style = if app.creator_focus == 4 {
        Style::default().fg(theme.text).bg(theme.highlight_bg)
    } else {
        Style::default().fg(theme.text_dim)
    };
    f.render_widget(
        Paragraph::new(format!("  Encryption:   [{}]", e2ee_label)).style(e2ee_style),
        fields[9],
    );

    let fed_label = if app.creator_federated { "Yes" } else { "No" };
    let fed_style = if app.creator_focus == 5 {
        Style::default().fg(theme.text).bg(theme.highlight_bg)
    } else {
        Style::default().fg(theme.text_dim)
    };
    f.render_widget(
        Paragraph::new(format!("  Federated:    [{}]", fed_label)).style(fed_style),
        fields[10],
    );

    f.render_widget(
        Paragraph::new("  Invite (comma-separated):").style(Style::default().fg(theme.text_dim)),
        fields[12],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.creator_invite)).style(s6),
        fields[13],
    );

    let hint = if let Some(err) = &app.creator_error {
        Paragraph::new(format!("  {}", err))
            .style(Style::default().fg(theme.status_err))
            .wrap(Wrap { trim: false })
    } else if app.creator_busy {
        Paragraph::new("  Creating room...")
            .style(Style::default().fg(theme.status_warn))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new("  Tab: next  Space: toggle  Enter: create  Esc: cancel")
            .style(Style::default().fg(theme.dimmed))
            .wrap(Wrap { trim: false })
    };
    f.render_widget(hint, fields[15]);

    if !app.creator_busy {
        let (row, col) = match app.creator_focus {
            1 => (fields[3].y, fields[3].x + 2 + app.creator_name.len() as u16),
            2 => (fields[6].y, fields[6].x + 2 + app.creator_topic.len() as u16),
            6 => (fields[13].y, fields[13].x + 2 + app.creator_invite.len() as u16),
            _ => return, // toggle/selector fields — no cursor
        };
        f.set_cursor_position((col, row));
    }
}

fn draw_editor_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let err_lines: u16 = if app.editor_error.is_some() { 2 } else { 1 };
    let height = (16 + err_lines).min(f.area().height);
    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Edit Room ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fields = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // room header
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // name field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // topic field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // invite field
            Constraint::Length(1), // spacer
            Constraint::Length(1), // leave button
            Constraint::Length(1), // delete button
            Constraint::Length(1), // spacer
            Constraint::Min(1),   // error/hint (wrapping)
        ])
        .split(inner);

    // Room header
    let room_name = app
        .editor_room_id
        .as_ref()
        .and_then(|rid| app.all_rooms.iter().find(|r| &r.id == rid))
        .map(|r| r.name.as_str())
        .unwrap_or("?");
    let via = app.editor_account_id.as_deref().unwrap_or("");
    f.render_widget(
        Paragraph::new(format!("  {} (via {})", room_name, via))
            .style(Style::default().fg(theme.accent)),
        fields[0],
    );

    let s0 = field_style(app.editor_focus == 0, theme);
    let s1 = field_style(app.editor_focus == 1, theme);
    let s2 = field_style(app.editor_focus == 2, theme);

    f.render_widget(
        Paragraph::new("  Room Name:").style(Style::default().fg(theme.text_dim)),
        fields[2],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.editor_name)).style(s0),
        fields[3],
    );
    f.render_widget(
        Paragraph::new("  Topic:").style(Style::default().fg(theme.text_dim)),
        fields[5],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.editor_topic)).style(s1),
        fields[6],
    );
    f.render_widget(
        Paragraph::new("  Invite User:").style(Style::default().fg(theme.text_dim)),
        fields[8],
    );
    f.render_widget(
        Paragraph::new(format!("  {}", app.editor_invite_user)).style(s2),
        fields[9],
    );

    // Leave button
    let leave_style = if app.editor_focus == 3 {
        if app.editor_confirm_leave {
            Style::default()
                .fg(theme.status_err)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.status_err)
                .bg(theme.highlight_bg)
        }
    } else {
        Style::default().fg(theme.status_err)
    };
    let leave_text = if app.editor_confirm_leave {
        "  [ Press Enter again to leave ]"
    } else {
        "  [ Leave Room ]"
    };
    f.render_widget(Paragraph::new(leave_text).style(leave_style), fields[11]);

    // Delete button
    let delete_style = if app.editor_focus == 4 {
        if app.editor_confirm_delete {
            Style::default()
                .fg(theme.status_err)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.status_err)
                .bg(theme.highlight_bg)
        }
    } else {
        Style::default().fg(theme.status_err)
    };
    let delete_text = if app.editor_confirm_delete {
        "  [ Press Enter again to DELETE ]"
    } else {
        "  [ Delete Room ]"
    };
    f.render_widget(Paragraph::new(delete_text).style(delete_style), fields[12]);

    let hint = if let Some(err) = &app.editor_error {
        Paragraph::new(format!("  {}", err))
            .style(Style::default().fg(theme.status_err))
            .wrap(Wrap { trim: false })
    } else if app.editor_busy {
        Paragraph::new("  Working...")
            .style(Style::default().fg(theme.status_warn))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new("  Tab: next  Enter: apply  Esc: back")
            .style(Style::default().fg(theme.dimmed))
            .wrap(Wrap { trim: false })
    };
    f.render_widget(hint, fields[14]);

    if !app.editor_busy {
        let (row, col) = match app.editor_focus {
            0 => (fields[3].y, fields[3].x + 2 + app.editor_name.len() as u16),
            1 => (fields[6].y, fields[6].x + 2 + app.editor_topic.len() as u16),
            2 => (fields[9].y, fields[9].x + 2 + app.editor_invite_user.len() as u16),
            _ => return, // buttons — no cursor
        };
        f.set_cursor_position((col, row));
    }
}

fn draw_recovery_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;

    // Calculate error height for wrapping
    let base_width = (f.area().width * 70 / 100).min(f.area().width);
    let err_lines: u16 = if let Some(err) = &app.recovery_error {
        let avail = base_width.saturating_sub(6) as usize; // borders + padding
        if avail == 0 { 1 } else { ((err.len() / avail) + 1).min(4) as u16 }
    } else {
        1
    };
    let height = 9 + err_lines; // 7 fixed rows + error area + borders

    let area = centered_rect(70, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Verify Session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),         // padding
            Constraint::Length(1),         // account id
            Constraint::Length(1),         // padding
            Constraint::Length(1),         // label
            Constraint::Length(1),         // input field
            Constraint::Length(1),         // padding
            Constraint::Length(err_lines), // error (wrapping)
            Constraint::Length(1),         // hint
        ])
        .split(inner);

    // Account ID
    let account_label = if app.recovery_account_idx < app.accounts.len() {
        app.accounts[app.recovery_account_idx].user_id.clone()
    } else {
        String::new()
    };
    f.render_widget(
        Paragraph::new(format!("  {}", account_label))
            .style(Style::default().fg(theme.accent)),
        rows[1],
    );

    // Label
    f.render_widget(
        Paragraph::new("  Recovery Key:").style(Style::default().fg(theme.text_dim)),
        rows[3],
    );

    // Input field
    let display_val = if app.recovery_key.is_empty() {
        " ".to_string()
    } else {
        app.recovery_key.clone()
    };
    f.render_widget(
        Paragraph::new(format!("  {}", display_val)).style(field_style(true, theme)),
        rows[4],
    );

    // Cursor
    let cursor_x = inner.x + 2 + app.recovery_key.len() as u16;
    f.set_cursor_position((cursor_x.min(inner.right().saturating_sub(1)), rows[4].y));

    // Error or busy
    if app.recovery_busy {
        f.render_widget(
            Paragraph::new("  Verifying...").style(Style::default().fg(theme.status_warn)),
            rows[6],
        );
    } else if let Some(err) = &app.recovery_error {
        f.render_widget(
            Paragraph::new(format!("  {}", err))
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(theme.status_err)),
            rows[6],
        );
    }

    // Hint
    f.render_widget(
        Paragraph::new("  Enter: verify  Esc: cancel")
            .style(Style::default().fg(theme.dimmed))
            .wrap(Wrap { trim: false }),
        rows[7],
    );
}

fn draw_message_action_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;

    let msg = app
        .selected_message
        .and_then(|idx| app.messages.get(idx));

    let preview_max = ((f.area().width as usize) * 50 / 100).saturating_sub(10);
    let msg_preview = msg
        .map(|m| {
            let body = m.body_text();
            let preview = if body.len() > preview_max && preview_max > 3 {
                format!("{}...", &body[..preview_max.saturating_sub(3)])
            } else {
                body.to_string()
            };
            format!("{}: {}", m.sender, preview)
        })
        .unwrap_or_default();

    if app.message_editing {
        // Edit mode: show text editor with character-exact wrapping
        let base_width = (f.area().width * 60 / 100).max(20).min(f.area().width);
        // inner width = base_width - 2 (borders), which is the actual widget width
        let widget_width = (base_width as usize).saturating_sub(2);
        let indent_w = 2usize; // "  " prefix
        let content_w = widget_width.saturating_sub(indent_w).max(1);
        let text_char_count = app.message_edit_text.chars().count();
        let text_lines = if text_char_count == 0 { 1 } else { (text_char_count + content_w - 1) / content_w };
        let edit_area_lines = text_lines.clamp(1, 10) as u16;
        let height = (8 + edit_area_lines).min(f.area().height);
        let area = centered_rect(60, height, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(" Edit Message ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),              // padding
                Constraint::Length(1),              // original preview
                Constraint::Length(1),              // padding
                Constraint::Length(1),              // label
                Constraint::Length(edit_area_lines), // input field (multi-line)
                Constraint::Length(1),              // padding
                Constraint::Min(1),                 // error or hint
            ])
            .split(inner);

        f.render_widget(
            Paragraph::new(format!("  {}", msg_preview))
                .style(Style::default().fg(theme.text_dim))
                .wrap(Wrap { trim: false }),
            rows[1],
        );

        f.render_widget(
            Paragraph::new("  New text:").style(Style::default().fg(theme.text_dim)),
            rows[3],
        );

        // Manual character-exact line breaking with consistent "  " indent
        let edit_cw = (rows[4].width as usize).saturating_sub(indent_w).max(1);
        let text_chars: Vec<char> = app.message_edit_text.chars().collect();
        let edit_lines: Vec<Line> = if text_chars.is_empty() {
            vec![Line::from("  ".to_string())]
        } else {
            text_chars
                .chunks(edit_cw)
                .map(|chunk| Line::from(format!("  {}", chunk.iter().collect::<String>())))
                .collect()
        };
        f.render_widget(
            Paragraph::new(edit_lines).style(field_style(true, theme)),
            rows[4],
        );

        // Cursor position: content wraps at edit_cw, each line prefixed with "  "
        let cursor_char_pos = app.message_edit_text[..app.message_edit_cursor].chars().count();
        let cursor_row = cursor_char_pos / edit_cw;
        let cursor_col = indent_w + cursor_char_pos % edit_cw;
        let cursor_x = rows[4].x + cursor_col as u16;
        let cursor_y = rows[4].y + cursor_row as u16;
        f.set_cursor_position((
            cursor_x.min(inner.right().saturating_sub(1)),
            cursor_y.min(rows[4].bottom().saturating_sub(1)),
        ));

        if app.message_edit_busy {
            f.render_widget(
                Paragraph::new("  Saving...").style(Style::default().fg(theme.status_warn)),
                rows[6],
            );
        } else if let Some(err) = &app.message_edit_error {
            f.render_widget(
                Paragraph::new(format!("  {}", err))
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(theme.status_err)),
                rows[6],
            );
        } else {
            f.render_widget(
                Paragraph::new("  Enter: save   Esc: back")
                    .style(Style::default().fg(theme.dimmed)),
                rows[6],
            );
        }
    } else {
        // Action menu: dynamic based on content type
        let actions = app.message_action_labels();
        let action_count = actions.len() as u16;
        let err_lines: u16 = if app.message_edit_error.is_some() { 1 } else { 0 };
        let height = (5 + action_count + 2 + err_lines).min(f.area().height);
        let area = centered_rect(50, height, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(" Message Actions ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut constraints: Vec<Constraint> = vec![
            Constraint::Length(1), // padding
            Constraint::Length(1), // message preview
            Constraint::Length(1), // padding
        ];
        for _ in 0..action_count {
            constraints.push(Constraint::Length(1)); // action row
        }
        constraints.push(Constraint::Length(1)); // padding
        constraints.push(Constraint::Min(1));    // error or hint

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        f.render_widget(
            Paragraph::new(format!("  {}", msg_preview))
                .style(Style::default().fg(theme.text_dim))
                .wrap(Wrap { trim: false }),
            rows[1],
        );

        for (i, action) in actions.iter().enumerate() {
            let is_sel = app.message_action_selected == i;
            let is_delete = *action == "Delete Message";
            let prefix = if is_sel { "  > " } else { "    " };
            let style = if is_sel {
                let fg = if is_delete { theme.status_err } else { theme.text };
                Style::default()
                    .fg(fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                let fg = if is_delete { theme.status_err } else { theme.text_dim };
                Style::default().fg(fg)
            };
            f.render_widget(
                Paragraph::new(format!("{}{}", prefix, action)).style(style),
                rows[3 + i],
            );
        }

        let hint_row = 3 + actions.len() + 1;
        if let Some(err) = &app.message_edit_error {
            f.render_widget(
                Paragraph::new(format!("  {}", err))
                    .style(Style::default().fg(theme.status_err))
                    .wrap(Wrap { trim: false }),
                rows[hint_row],
            );
        } else {
            f.render_widget(
                Paragraph::new("  Enter: select  Esc: cancel")
                    .style(Style::default().fg(theme.dimmed))
                    .wrap(Wrap { trim: false }),
                rows[hint_row],
            );
        }
    }
}

fn field_style(focused: bool, theme: &Theme) -> Style {
    if focused {
        Style::default().fg(theme.text).bg(theme.highlight_bg)
    } else {
        Style::default().fg(theme.text_dim)
    }
}

fn draw_sas_verify_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;

    let account_label = app
        .accounts
        .get(app.sas_account_idx)
        .map(|a| a.user_id.as_str())
        .unwrap_or("unknown");

    match app.sas_state {
        SasOverlayState::Waiting => {
            let area = centered_rect(55, 8, f.area());
            f.render_widget(Clear, area);
            let block = Block::default()
                .title(" Verify from Device ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent));
            let inner = block.inner(area);
            f.render_widget(block, area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // padding
                    Constraint::Length(1), // account
                    Constraint::Length(1), // padding
                    Constraint::Min(2),   // message
                    Constraint::Length(1), // hint
                ])
                .split(inner);

            f.render_widget(
                Paragraph::new(format!("  {}", account_label))
                    .style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
                    .wrap(Wrap { trim: false }),
                rows[1],
            );
            f.render_widget(
                Paragraph::new("  Waiting for another device to accept...")
                    .style(Style::default().fg(theme.text_dim))
                    .wrap(Wrap { trim: false }),
                rows[3],
            );
            f.render_widget(
                Paragraph::new("  Esc: cancel")
                    .style(Style::default().fg(theme.dimmed))
                    .wrap(Wrap { trim: false }),
                rows[4],
            );
        }

        SasOverlayState::Incoming => {
            let area = centered_rect(55, 8, f.area());
            f.render_widget(Clear, area);
            let block = Block::default()
                .title(" Verification Request ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent));
            let inner = block.inner(area);
            f.render_widget(block, area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // padding
                    Constraint::Min(2),   // message
                    Constraint::Length(1), // padding
                    Constraint::Length(1), // hint
                ])
                .split(inner);

            let requester = app.sas_user_id.as_deref().unwrap_or("Another device");
            f.render_widget(
                Paragraph::new(format!("  {} wants to verify this session.", requester))
                    .style(Style::default().fg(theme.text))
                    .wrap(Wrap { trim: false }),
                rows[1],
            );
            f.render_widget(
                Paragraph::new("  Enter: accept  Esc: decline")
                    .style(Style::default().fg(theme.dimmed))
                    .wrap(Wrap { trim: false }),
                rows[3],
            );
        }

        SasOverlayState::Emojis => {
            let term = f.area();
            let emoji_count = app.sas_emojis.len();
            let emoji_field_width: usize = 6; // wider spacing between emojis

            // Compute how many emojis fit per row based on available inner width
            let base_width = (term.width * 70 / 100).max(20).min(term.width);
            let inner_w = (base_width as usize).saturating_sub(4); // borders + padding
            let emojis_per_row = (inner_w / emoji_field_width).max(1).min(emoji_count.max(1));
            let emoji_rows = if emoji_count == 0 { 1 } else { (emoji_count + emojis_per_row - 1) / emojis_per_row };

            let err_lines: u16 = if app.sas_error.is_some() { 2 } else { 0 };
            // Each emoji row gets 3 lines: blank + emojis + blank (spacing)
            let emoji_area = (emoji_rows as u16) * 3;
            // Layout: 1 pad + 2 instruction + emoji_area + 1 pad + 1 hint + err
            let height = (5 + emoji_area + err_lines).min(term.height);

            let area = centered_rect(70, height, term);
            f.render_widget(Clear, area);
            let block = Block::default()
                .title(" Verify Emojis ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent));
            let inner = block.inner(area);
            f.render_widget(block, area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),          // padding
                    Constraint::Length(2),          // instruction
                    Constraint::Length(emoji_area), // emoji rows with spacing
                    Constraint::Length(1),          // padding
                    Constraint::Length(1),          // hint
                    Constraint::Min(0),            // error
                ])
                .split(inner);

            f.render_widget(
                Paragraph::new("  Confirm these emojis match your other device:")
                    .style(Style::default().fg(theme.text))
                    .wrap(Wrap { trim: false }),
                rows[1],
            );

            // Build emoji lines with generous spacing — no description text
            let mut emoji_lines: Vec<Line> = Vec::new();

            for chunk in app.sas_emojis.chunks(emojis_per_row) {
                emoji_lines.push(Line::from("")); // blank line above
                let emoji_str: String = chunk.iter()
                    .map(|(symbol, _)| format!(" {}  ", symbol))
                    .collect::<Vec<_>>()
                    .join("");
                emoji_lines.push(Line::from(Span::styled(
                    format!("    {}", emoji_str),
                    Style::default().fg(theme.text),
                )));
                emoji_lines.push(Line::from("")); // blank line below
            }

            f.render_widget(
                Paragraph::new(emoji_lines),
                rows[2],
            );
            f.render_widget(
                Paragraph::new("  y/Enter: match  n: mismatch  Esc: cancel")
                    .style(Style::default().fg(theme.dimmed))
                    .wrap(Wrap { trim: false }),
                rows[4],
            );

            if let Some(err) = &app.sas_error {
                f.render_widget(
                    Paragraph::new(format!("  {}", err))
                        .style(Style::default().fg(theme.status_err))
                        .wrap(Wrap { trim: false }),
                    rows[5],
                );
            }
        }

        SasOverlayState::Confirming => {
            let area = centered_rect(55, 6, f.area());
            f.render_widget(Clear, area);
            let block = Block::default()
                .title(" Verifying... ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent));
            let inner = block.inner(area);
            f.render_widget(block, area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(inner);

            f.render_widget(
                Paragraph::new("  Waiting for other device to confirm...")
                    .style(Style::default().fg(theme.status_warn))
                    .wrap(Wrap { trim: false }),
                rows[1],
            );
        }

        SasOverlayState::Done => {
            let area = centered_rect(55, 6, f.area());
            f.render_widget(Clear, area);
            let block = Block::default()
                .title(" Verified ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.status_ok));
            let inner = block.inner(area);
            f.render_widget(block, area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(inner);

            f.render_widget(
                Paragraph::new("  Session verified successfully!")
                    .style(Style::default().fg(theme.status_ok).add_modifier(Modifier::BOLD))
                    .wrap(Wrap { trim: false }),
                rows[1],
            );
            f.render_widget(
                Paragraph::new("  Enter/Esc: close")
                    .style(Style::default().fg(theme.dimmed))
                    .wrap(Wrap { trim: false }),
                rows[2],
            );
        }

        SasOverlayState::Failed => {
            let area = centered_rect(55, 8, f.area());
            f.render_widget(Clear, area);
            let block = Block::default()
                .title(" Verification Failed ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.status_err));
            let inner = block.inner(area);
            f.render_widget(block, area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(2),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .split(inner);

            let reason = app.sas_error.as_deref().unwrap_or("Verification cancelled");
            f.render_widget(
                Paragraph::new(format!("  {}", reason))
                    .style(Style::default().fg(theme.status_err))
                    .wrap(Wrap { trim: false }),
                rows[1],
            );
            f.render_widget(
                Paragraph::new("  Enter/Esc: close")
                    .style(Style::default().fg(theme.dimmed))
                    .wrap(Wrap { trim: false }),
                rows[3],
            );
        }
    }
}

fn draw_emoji_picker_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    const EMOJIS: &[&str] = &[
        "\u{1F44D}", "\u{2764}\u{FE0F}", "\u{1F602}", "\u{1F62E}",
        "\u{1F622}", "\u{1F389}", "\u{1F525}", "\u{1F440}",
    ];
    let term = f.area();
    let height: u16 = 5;
    let area = centered_rect(50, height, term);

    f.render_widget(ratatui::widgets::Clear, area);
    let block = Block::default()
        .title(Span::styled(
            " React ",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let emoji_spans: Vec<Span> = EMOJIS
        .iter()
        .enumerate()
        .flat_map(|(i, emoji)| {
            let style = if i == app.emoji_picker_selected {
                Style::default().bg(theme.highlight_bg)
            } else {
                Style::default()
            };
            vec![
                Span::styled(format!(" {} ", emoji), style),
                Span::raw(" "),
            ]
        })
        .collect();

    let emoji_line = Line::from(emoji_spans);
    let hint = Line::from(Span::styled(
        "  \u{2190}/\u{2192}: select   Enter: send   Esc: cancel",
        Style::default().fg(theme.text_dim),
    ));

    let content = Paragraph::new(vec![
        Line::from(""),
        emoji_line,
        hint,
    ]);
    f.render_widget(content, inner);
}

fn draw_room_info_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let details = match &app.room_details {
        Some(d) => d,
        None => return,
    };

    let term = f.area();
    let topic_lines: u16 = if let Some(ref topic) = details.topic {
        let inner_w = (term.width * 60 / 100).saturating_sub(4) as usize;
        if inner_w == 0 { 1 } else { ((topic.len().max(1) + inner_w - 1) / inner_w) as u16 }
    } else {
        0
    };
    let height = (9 + topic_lines).min(term.height);
    let area = centered_rect(60, height, term);

    f.render_widget(ratatui::widgets::Clear, area);
    let block = Block::default()
        .title(Span::styled(
            " Room Info ",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  # {}", details.name),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("  {}", details.room_id),
            Style::default().fg(theme.text_dim),
        )),
        Line::from(""),
    ];

    if let Some(ref topic) = details.topic {
        lines.push(Line::from(Span::styled(
            format!("  Topic: {}", topic),
            Style::default().fg(theme.text),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        format!("  Members: {}", details.member_count),
        Style::default().fg(theme.text),
    )));
    lines.push(Line::from(Span::styled(
        format!("  Encryption: {}", details.encryption),
        Style::default().fg(theme.text),
    )));

    let content = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(content, inner);
}

fn draw_file_confirm_overlay(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let path_str = app.pending_file_drop.as_deref().unwrap_or("");
    let filename = std::path::Path::new(path_str)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path_str);

    // File size
    let size_label = std::fs::metadata(path_str)
        .map(|m| {
            let bytes = m.len();
            if bytes >= 1_048_576 {
                format!("{:.1} MB", bytes as f64 / 1_048_576.0)
            } else if bytes >= 1024 {
                format!("{:.1} KB", bytes as f64 / 1024.0)
            } else {
                format!("{} B", bytes)
            }
        })
        .unwrap_or_default();

    let height: u16 = 7;
    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Send File ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // padding
            Constraint::Length(1), // filename
            Constraint::Length(1), // padding
            Constraint::Length(1), // hint
            Constraint::Min(0),
        ])
        .split(inner);

    let file_info = if size_label.is_empty() {
        format!("  {}", filename)
    } else {
        format!("  {} ({})", filename, size_label)
    };
    f.render_widget(
        Paragraph::new(file_info).style(Style::default().fg(theme.text)),
        rows[1],
    );

    f.render_widget(
        Paragraph::new("  Enter: send    Esc: cancel")
            .style(Style::default().fg(theme.dimmed)),
        rows[3],
    );
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = (area.width * percent_x / 100).min(area.width);
    let popup_height = height.min(area.height);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    Rect::new(area.x + x, area.y + y, popup_width, popup_height)
}
