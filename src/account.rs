use anyhow::Result;
use matrix_sdk::{
    Client, Room, SessionMeta, SessionTokens,
    authentication::matrix::MatrixSession,
    config::SyncSettings,
    room::MessagesOptions,
    ruma::{
        OwnedRoomId, OwnedUserId, UserId,
        events::{
            AnySyncMessageLikeEvent, AnySyncTimelineEvent,
            room::message::{
                MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
                SyncRoomMessageEvent,
            },
        },
    },
};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::info;

use crate::config::{SavedAccount, data_dir};

/// Events pushed from Matrix sync to the UI
#[derive(Debug, Clone)]
pub enum MatrixEvent {
    Message {
        room_id: OwnedRoomId,
        sender: OwnedUserId,
        body: String,
        timestamp: u64,
    },
    RoomsUpdated {
        account_id: String,
    },
    SyncError {
        account_id: String,
        error: String,
    },
    SyncComplete {
        account_id: String,
    },
}

/// Room info for display
#[derive(Debug, Clone)]
pub struct RoomInfo {
    pub id: OwnedRoomId,
    pub name: String,
    pub is_dm: bool,
    pub unread: u64,
    pub account_id: String,
}

/// A single logged-in Matrix account
pub struct Account {
    pub client: Client,
    pub user_id: String,
    pub homeserver: String,
    pub display_name: String,
    pub syncing: bool,
    pub sync_complete: bool,
    sync_handle: Option<JoinHandle<()>>,
}

impl Account {
    /// Login with username and password
    pub async fn login(
        homeserver: &str,
        username: &str,
        password: &str,
    ) -> Result<(Self, SavedAccount)> {
        let url = normalize_homeserver(homeserver);
        // Normalize to @user:server format so db path matches restore()
        let normalized_id = if username.starts_with('@') {
            username.to_string()
        } else {
            format!("@{}:{}", username, homeserver)
        };
        let db_path = session_db_path(&normalized_id, homeserver);
        std::fs::create_dir_all(&db_path)?;

        let client = Client::builder()
            .homeserver_url(&url)
            .sqlite_store(&db_path, None)
            .build()
            .await?;

        let response = client
            .matrix_auth()
            .login_username(username, password)
            .initial_device_display_name("MatrixTUI")
            .await?;

        let user_id = response.user_id.to_string();
        let saved = SavedAccount {
            homeserver: homeserver.to_string(),
            user_id: user_id.clone(),
            access_token: response.access_token,
            device_id: response.device_id.to_string(),
        };

        let account = Self {
            display_name: username.to_string(),
            user_id,
            homeserver: homeserver.to_string(),
            client,
            syncing: false,
            sync_complete: false,
            sync_handle: None,
        };

        Ok((account, saved))
    }

    /// Restore from saved session
    pub async fn restore(saved: &SavedAccount) -> Result<Self> {
        let url = normalize_homeserver(&saved.homeserver);
        let db_path = session_db_path(&saved.user_id, &saved.homeserver);
        std::fs::create_dir_all(&db_path)?;

        let client = Client::builder()
            .homeserver_url(&url)
            .sqlite_store(&db_path, None)
            .build()
            .await?;

        let session = MatrixSession {
            meta: SessionMeta {
                user_id: <&UserId>::try_from(saved.user_id.as_str())?.to_owned(),
                device_id: saved.device_id.as_str().into(),
            },
            tokens: SessionTokens {
                access_token: saved.access_token.clone(),
                refresh_token: None,
            },
        };
        client.restore_session(session).await?;

        Ok(Self {
            display_name: saved.user_id.clone(),
            user_id: saved.user_id.clone(),
            homeserver: saved.homeserver.clone(),
            client,
            syncing: false,
            sync_complete: false,
            sync_handle: None,
        })
    }

    /// Start background sync, push events to channel
    pub fn start_sync(&mut self, tx: mpsc::UnboundedSender<MatrixEvent>) {
        if self.syncing {
            return;
        }
        self.syncing = true;
        let client = self.client.clone();
        let account_id = self.user_id.clone();

        let handle = tokio::spawn(async move {
            info!("Starting sync for {}", account_id);

            // Register message handler
            let tx_msg = tx.clone();
            let aid = account_id.clone();
            client.add_event_handler(
                move |event: OriginalSyncRoomMessageEvent, room: Room| {
                    let tx = tx_msg.clone();
                    let aid = aid.clone();
                    async move {
                        let body = match &event.content.msgtype {
                            MessageType::Text(text) => text.body.clone(),
                            MessageType::Image(_) => "[image]".to_string(),
                            MessageType::File(f) => format!("[file: {}]", f.filename()),
                            MessageType::Video(_) => "[video]".to_string(),
                            MessageType::Audio(_) => "[audio]".to_string(),
                            MessageType::Notice(n) => n.body.clone(),
                            MessageType::Emote(e) => format!("* {}", e.body),
                            _ => "[unsupported message type]".to_string(),
                        };
                        let _ = tx.send(MatrixEvent::Message {
                            room_id: room.room_id().to_owned(),
                            sender: event.sender,
                            body,
                            timestamp: event
                                .origin_server_ts
                                .as_secs()
                                .into(),
                        });
                        let _ = tx.send(MatrixEvent::RoomsUpdated { account_id: aid });
                    }
                },
            );

            // Initial sync
            let settings = SyncSettings::default();
            match client.sync_once(settings.clone()).await {
                Ok(_) => {
                    let _ = tx.send(MatrixEvent::SyncComplete {
                        account_id: account_id.clone(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(MatrixEvent::SyncError {
                        account_id: account_id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            }

            // Continuous sync
            let _ = client.sync(settings).await;
        });
        self.sync_handle = Some(handle);
    }

    /// Stop the background sync task
    pub fn stop_sync(&mut self) {
        if let Some(handle) = self.sync_handle.take() {
            handle.abort();
        }
        self.syncing = false;
    }

    /// Get joined rooms as RoomInfo
    pub async fn rooms(&self) -> Vec<RoomInfo> {
        let mut result = Vec::new();
        for room in self.client.joined_rooms() {
            let name = room
                .cached_display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| room.room_id().to_string());
            let is_dm = room.is_direct().await.unwrap_or(false);
            result.push(RoomInfo {
                id: room.room_id().to_owned(),
                name,
                is_dm,
                unread: room.num_unread_notifications().into(),
                account_id: self.user_id.clone(),
            });
        }
        result
    }

    /// Fetch recent message history for a room
    pub async fn fetch_history(
        &self,
        room_id: &OwnedRoomId,
        limit: u32,
    ) -> Result<Vec<crate::app::DisplayMessage>> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;

        let prev_batch = room.last_prev_batch();
        info!(
            "fetch_history for {} — prev_batch: {:?}",
            room_id,
            prev_batch.as_deref().unwrap_or("None")
        );

        let mut options = MessagesOptions::backward();
        if prev_batch.is_some() {
            options = options.from(prev_batch.as_deref());
        }

        let response = room.messages(options).await?;
        info!(
            "fetch_history got {} events, end token: {:?}",
            response.chunk.len(),
            response.end
        );
        let mut messages = Vec::new();

        for timeline_event in &response.chunk {
            match timeline_event.raw().deserialize() {
                Ok(AnySyncTimelineEvent::MessageLike(
                    AnySyncMessageLikeEvent::RoomMessage(SyncRoomMessageEvent::Original(original)),
                )) => {
                    let body = match &original.content.msgtype {
                        MessageType::Text(text) => text.body.clone(),
                        MessageType::Image(_) => "[image]".to_string(),
                        MessageType::File(f) => format!("[file: {}]", f.filename()),
                        MessageType::Video(_) => "[video]".to_string(),
                        MessageType::Audio(_) => "[audio]".to_string(),
                        MessageType::Notice(n) => n.body.clone(),
                        MessageType::Emote(e) => format!("* {}", e.body),
                        _ => "[unsupported message type]".to_string(),
                    };
                    messages.push(crate::app::DisplayMessage {
                        sender: original.sender.to_string(),
                        body,
                        timestamp: original.origin_server_ts.as_secs().into(),
                    });
                }
                Ok(_) => {} // state events, reactions, etc — skip
                Err(e) => {
                    // Likely an encrypted message that couldn't be decrypted
                    info!("Failed to deserialize event: {}", e);
                    messages.push(crate::app::DisplayMessage {
                        sender: "".to_string(),
                        body: "[encrypted message — unable to decrypt]".to_string(),
                        timestamp: 0,
                    });
                }
            }
        }

        // Messages come newest-first from backward pagination, reverse for chronological
        messages.reverse();

        // Only keep the last `limit` messages
        if messages.len() > limit as usize {
            messages = messages.split_off(messages.len() - limit as usize);
        }

        Ok(messages)
    }

    /// Send a text message to a room
    pub async fn send_message(&self, room_id: &OwnedRoomId, body: &str) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found for {}", self.user_id))?;
        info!("Sending to {} via {}", room_id, self.user_id);
        let content = RoomMessageEventContent::text_plain(body);
        room.send(content).await?;
        info!("Send OK");
        Ok(())
    }

    /// Get current display name from the server
    pub async fn get_display_name(&self) -> Result<Option<String>> {
        let name = self.client.account().get_display_name().await?;
        Ok(name)
    }

    /// Set display name
    pub async fn set_display_name(&self, name: &str) -> Result<()> {
        self.client.account().set_display_name(Some(name)).await?;
        Ok(())
    }

    /// Get current avatar MXC URL
    pub async fn get_avatar_url(&self) -> Result<Option<String>> {
        let url = self.client.account().get_avatar_url().await?;
        Ok(url.map(|u| u.to_string()))
    }

    /// Set avatar by MXC URL
    pub async fn set_avatar_url(&self, mxc_url: &str) -> Result<()> {
        use matrix_sdk::ruma::OwnedMxcUri;
        let uri: OwnedMxcUri = mxc_url.into();
        self.client.account().set_avatar_url(Some(&uri)).await?;
        Ok(())
    }

    /// Upload avatar from local file path
    pub async fn upload_avatar(&self, file_path: &str) -> Result<String> {
        let path = std::path::Path::new(file_path);
        let data = std::fs::read(path)?;
        let mime = mime_from_extension(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
        let response = self.client.account().upload_avatar(&mime, data).await?;
        Ok(response.to_string())
    }

    /// Create a room
    pub async fn create_room(
        &self,
        name: Option<&str>,
        topic: Option<&str>,
        is_public: bool,
        e2ee: bool,
        invite_ids: Vec<String>,
    ) -> Result<OwnedRoomId> {
        use matrix_sdk::ruma::api::client::room::{
            create_room::v3::{Request, RoomPreset},
            Visibility,
        };

        let mut request = Request::new();
        if let Some(n) = name {
            request.name = Some(n.to_string());
        }
        if let Some(t) = topic {
            request.topic = Some(t.to_string());
        }
        request.visibility = if is_public {
            Visibility::Public
        } else {
            Visibility::Private
        };
        request.preset = Some(if is_public {
            RoomPreset::PublicChat
        } else if e2ee {
            RoomPreset::TrustedPrivateChat
        } else {
            RoomPreset::PrivateChat
        });

        let mut invites = Vec::new();
        for id_str in &invite_ids {
            let trimmed = id_str.trim();
            if !trimmed.is_empty() {
                let user_id = <&UserId>::try_from(trimmed)?.to_owned();
                invites.push(user_id);
            }
        }
        request.invite = invites;

        let response = self.client.create_room(request).await?;
        Ok(response.room_id().to_owned())
    }

    /// Set room name
    pub async fn set_room_name(&self, room_id: &OwnedRoomId, name: &str) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        room.set_name(name.to_string()).await?;
        Ok(())
    }

    /// Set room topic
    pub async fn set_room_topic(&self, room_id: &OwnedRoomId, topic: &str) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        room.set_room_topic(topic).await?;
        Ok(())
    }

    /// Invite a user to a room
    pub async fn invite_user(&self, room_id: &OwnedRoomId, user_id_str: &str) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let user_id = <&UserId>::try_from(user_id_str)?;
        room.invite_user_by_id(user_id).await?;
        Ok(())
    }

    /// Leave a room
    pub async fn leave_room(&self, room_id: &OwnedRoomId) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        room.leave().await?;
        Ok(())
    }

    /// Leave and forget a room (removes from room list permanently)
    pub async fn forget_room(&self, room_id: &OwnedRoomId) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        room.leave().await?;
        room.forget().await?;
        Ok(())
    }

    /// Get room topic (from cached state)
    pub fn get_room_topic(&self, room_id: &OwnedRoomId) -> Option<String> {
        let room = self.client.get_room(room_id)?;
        room.topic()
    }

    /// Recover E2EE secrets using a recovery key (or passphrase)
    pub async fn recover_with_key(&self, recovery_key: &str) -> Result<()> {
        self.client
            .encryption()
            .recovery()
            .recover(recovery_key)
            .await?;
        Ok(())
    }

    /// Check if session has complete cross-signing keys
    pub async fn is_verified(&self) -> bool {
        self.client
            .encryption()
            .cross_signing_status()
            .await
            .map(|s| s.is_complete())
            .unwrap_or(false)
    }
}

impl Drop for Account {
    fn drop(&mut self) {
        if let Some(handle) = self.sync_handle.take() {
            handle.abort();
        }
    }
}

fn mime_from_extension(ext: &str) -> mime::Mime {
    match ext.to_lowercase().as_str() {
        "png" => "image/png".parse().unwrap(),
        "jpg" | "jpeg" => "image/jpeg".parse().unwrap(),
        "gif" => "image/gif".parse().unwrap(),
        "webp" => "image/webp".parse().unwrap(),
        "svg" => "image/svg+xml".parse().unwrap(),
        _ => "application/octet-stream".parse().unwrap(),
    }
}

fn normalize_homeserver(hs: &str) -> String {
    if hs.starts_with("http://") || hs.starts_with("https://") {
        hs.to_string()
    } else {
        format!("https://{}", hs)
    }
}

fn session_db_path(user_id: &str, _homeserver: &str) -> PathBuf {
    let safe_id = user_id.replace(['@', ':', '.'], "_");
    data_dir().join("sessions").join(safe_id)
}
