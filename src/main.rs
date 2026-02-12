mod account;
mod app;
mod config;
mod event;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    cursor::SetCursorStyle,
    event::{DisableMouseCapture, EnableMouseCapture, EnableBracketedPaste, DisableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use ratatui_image::picker::Picker;
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging to file (don't pollute the TUI)
    let log_dir = config::data_dir();
    std::fs::create_dir_all(&log_dir)?;
    let log_file = std::fs::File::create(log_dir.join("matrixtui.log"))?;
    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    // Load config and saved accounts
    let cfg = config::Config::load()?;

    // Detect terminal graphics protocol BEFORE raw mode (query needs normal terminal)
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::from_fontsize((8, 16)));

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, SetCursorStyle::SteadyBar, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let mut app = App::new(cfg, picker);
    app.restore_sessions().await;
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        SetCursorStyle::DefaultUserShape,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    result
}
