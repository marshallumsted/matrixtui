use crossterm::event::{self, Event, KeyEvent};
use matrix_sdk::ruma::OwnedRoomId;
use ratatui_image::protocol::StatefulProtocol;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::account::MatrixEvent;

/// All events the app loop handles
pub enum AppEvent {
    /// A key was pressed
    Key(KeyEvent),
    /// Terminal was resized
    Resize,
    /// A Matrix event from any account
    Matrix(MatrixEvent),
    /// Bracketed paste data
    Paste(String),
    /// An image has been downloaded and decoded, ready for display
    ImageReady {
        room_id: OwnedRoomId,
        event_id: String,
        protocol: Arc<Mutex<StatefulProtocol>>,
    },
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
                    Ok(Event::Resize(_, _)) => {
                        let _ = tx.send(AppEvent::Resize);
                    }
                    Ok(Event::Paste(data)) => {
                        let _ = tx.send(AppEvent::Paste(data));
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
