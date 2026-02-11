use crossterm::event::{self, Event};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::account::MatrixEvent;

/// All events the app loop handles
#[derive(Debug)]
pub enum AppEvent {
    /// A key was pressed
    Key(KeyEvent),
    /// Terminal was resized
    Resize(u16, u16),
    /// A Matrix event from any account
    Matrix(MatrixEvent),
    /// Tick for periodic UI refresh
    Tick,
}

/// Spawns a task that reads terminal events and forwards them
pub fn spawn_input_reader(tx: mpsc::UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        loop {
            // Poll for crossterm events with a timeout (tick rate)
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key)) => {
                        let _ = tx.send(AppEvent::Key(key));
                    }
                    Ok(Event::Resize(w, h)) => {
                        let _ = tx.send(AppEvent::Resize(w, h));
                    }
                    _ => {}
                }
            } else {
                let _ = tx.send(AppEvent::Tick);
            }
        }
    });
}

/// Spawns a bridge that forwards MatrixEvents into AppEvents
pub fn spawn_matrix_bridge(
    mut matrix_rx: mpsc::UnboundedReceiver<MatrixEvent>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = matrix_rx.recv().await {
            let _ = app_tx.send(AppEvent::Matrix(event));
        }
    });
}
