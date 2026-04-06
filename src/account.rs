use anyhow::Result;
use matrix_sdk::{
    Client, Room, SessionMeta, SessionTokens,
    authentication::matrix::MatrixSession,
    config::SyncSettings,
    encryption::{
        BackupDownloadStrategy, EncryptionSettings,
        verification::{SasVerification, VerificationRequest, VerificationRequestState},
    },
    media::{MediaFormat, MediaRequestParameters},
    room::MessagesOptions,
    ruma::{
        OwnedEventId, OwnedRoomId, OwnedUserId, UInt, UserId,
        api::client::receipt::create_receipt,
        events::{
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncEphemeralRoomEvent,
            key::verification::VerificationMethod,
            reaction::OriginalSyncReactionEvent,
            receipt::ReceiptThread,
            relation::Annotation,
            room::message::{
                AddMentions, ForwardThread, MessageType, OriginalSyncRoomMessageEvent,
                Relation, ReplyMetadata, RoomMessageEventContent,
                RoomMessageEventContentWithoutRelation, SyncRoomMessageEvent,
            },
            room::MediaSource,
            typing::TypingEventContent,
                presence::PresenceEvent,
        },
    },
};
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::info;

use crate::config::{SavedAccount, data_dir};

/// Strip the Matrix reply fallback from a message body.
/// Reply bodies look like: "> <@user:server> quoted text\n> more\n\nActual reply"
/// This strips the leading `> ` lines and the blank line separator.
fn strip_reply_fallback(body: &str) -> String {
    let mut lines = body.lines().peekable();
    // Skip lines starting with "> "
    while let Some(line) = lines.peek() {
        if line.starts_with("> ") {
            lines.next();
        } else {
            break;
        }
    }
    // Skip the blank separator line
    if let Some(line) = lines.peek() {
        if line.is_empty() {
            lines.next();
        }
    }
    let remaining: String = lines.collect::<Vec<_>>().join("\n");
    if remaining.is_empty() {
        // Fallback: return original if stripping removed everything
        body.to_string()
    } else {
        remaining
    }
}

/// Events pushed from Matrix sync to the UI
#[derive(Debug, Clone)]
pub enum MatrixEvent {
    Message {
        room_id: OwnedRoomId,
        sender: OwnedUserId,
        body: String,
        timestamp: u64,
        event_id: String,
        reply_to_event_id: Option<String>,
    },
    Typing {
        room_id: OwnedRoomId,
        user_ids: Vec<OwnedUserId>,
    },
    Reaction {
        room_id: OwnedRoomId,
        event_id: String,
        key: String,
    },
    Presence {
        user_id: String,
        presence: String,
    },
    RoomsUpdated,
    SyncError {
        account_id: String,
        error: String,
    },
    SyncComplete {
        account_id: String,
    },
    KeysDownloaded {
        room_id: OwnedRoomId,
        account_id: String,
    },
    VerificationIncoming {
        account_id: String,
        user_id: String,
        flow_id: String,
    },
    SasStarted {
        flow_id: String,
        sas: SasVerification,
    },
    SasEmojis {
        flow_id: String,
        emojis: Vec<(String, String)>, // (symbol, description)
    },
    SasDone {
        flow_id: String,
    },
    SasCancelled {
        flow_id: String,
        reason: String,
    },
    ImageMessage {
        room_id: OwnedRoomId,
        sender: OwnedUserId,
        timestamp: u64,
        event_id: String,
        body: String,
        source: MediaSource,
        reply_to_event_id: Option<String>,
    },
    FileMessage {
        room_id: OwnedRoomId,
        sender: OwnedUserId,
        timestamp: u64,
        event_id: String,
        body: String,
        source: MediaSource,
        media_type: crate::app::FileKind,
        reply_to_event_id: Option<String>,
    },
    OlderMessagesLoaded {
        room_id: OwnedRoomId,
        messages: Vec<crate::app::DisplayMessage>,
        next_token: Option<String>,
    },
    OlderMessagesFailed {
        error: String,
    },
}

/// Room info for display
#[derive(Debug, Clone)]
pub struct RoomInfo {
    pub id: OwnedRoomId,
    pub name: String,
    pub is_dm: bool,
    pub encrypted: bool,
    pub is_space: bool,
    pub parent_space_id: Option<OwnedRoomId>,
    pub unread: u64,
    pub account_id: String,
}

/// A room member for display
#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub user_id: String,
    pub display_name: String,
}

/// Detailed room info for the Room Info overlay
#[derive(Debug, Clone)]
pub struct RoomDetails {
    pub name: String,
    pub topic: Option<String>,
    pub member_count: u64,
    pub encryption: String,
    pub room_id: String,
    pub members: Vec<MemberInfo>,
}

/// Space details for the Space Home overlay
#[derive(Debug, Clone)]
pub struct SpaceDetails {
    pub name: String,
    pub topic: Option<String>,
    pub member_count: u64,
    pub room_id: String,
    pub rooms: Vec<SpaceChildInfo>,
}

/// A child room/space within a space
#[derive(Debug, Clone)]
pub struct SpaceChildInfo {
    pub name: String,
    pub is_space: bool,
    pub encrypted: bool,
    pub member_count: u64,
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
            .with_encryption_settings(e2ee_settings())
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
            .with_encryption_settings(e2ee_settings())
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
            client.add_event_handler(
                move |event: OriginalSyncRoomMessageEvent, room: Room| {
                    let tx = tx_msg.clone();
                    async move {
                        let reply_to_event_id = match &event.content.relates_to {
                            Some(Relation::Reply { in_reply_to }) => {
                                Some(in_reply_to.event_id.to_string())
                            }
                            _ => None,
                        };
                        // Handle image messages separately
                        if let MessageType::Image(ref img) = event.content.msgtype {
                            let _ = tx.send(MatrixEvent::ImageMessage {
                                room_id: room.room_id().to_owned(),
                                sender: event.sender.clone(),
                                timestamp: event.origin_server_ts.as_secs().into(),
                                event_id: event.event_id.to_string(),
                                body: img.filename().to_string(),
                                source: img.source.clone(),
                                reply_to_event_id,
                            });
                            let _ = tx.send(MatrixEvent::RoomsUpdated);
                            return;
                        }
                        // Handle file/video/audio messages separately
                        match &event.content.msgtype {
                            MessageType::File(f) => {
                                let _ = tx.send(MatrixEvent::FileMessage {
                                    room_id: room.room_id().to_owned(),
                                    sender: event.sender.clone(),
                                    timestamp: event.origin_server_ts.as_secs().into(),
                                    event_id: event.event_id.to_string(),
                                    body: f.filename().to_string(),
                                    source: f.source.clone(),
                                    media_type: crate::app::FileKind::File,
                                    reply_to_event_id,
                                });
                                let _ = tx.send(MatrixEvent::RoomsUpdated);
                                return;
                            }
                            MessageType::Video(v) => {
                                let _ = tx.send(MatrixEvent::FileMessage {
                                    room_id: room.room_id().to_owned(),
                                    sender: event.sender.clone(),
                                    timestamp: event.origin_server_ts.as_secs().into(),
                                    event_id: event.event_id.to_string(),
                                    body: v.filename().to_string(),
                                    source: v.source.clone(),
                                    media_type: crate::app::FileKind::Video,
                                    reply_to_event_id,
                                });
                                let _ = tx.send(MatrixEvent::RoomsUpdated);
                                return;
                            }
                            MessageType::Audio(a) => {
                                let _ = tx.send(MatrixEvent::FileMessage {
                                    room_id: room.room_id().to_owned(),
                                    sender: event.sender.clone(),
                                    timestamp: event.origin_server_ts.as_secs().into(),
                                    event_id: event.event_id.to_string(),
                                    body: a.filename().to_string(),
                                    source: a.source.clone(),
                                    media_type: crate::app::FileKind::Audio,
                                    reply_to_event_id,
                                });
                                let _ = tx.send(MatrixEvent::RoomsUpdated);
                                return;
                            }
                            _ => {}
                        }
                        let body = match &event.content.msgtype {
                            MessageType::Text(text) => text.body.clone(),
                            MessageType::Notice(n) => n.body.clone(),
                            MessageType::Emote(e) => format!("* {}", e.body),
                            _ => "[unsupported message type]".to_string(),
                        };
                        // Strip reply fallback from body if this is a reply
                        let body = if reply_to_event_id.is_some() {
                            strip_reply_fallback(&body)
                        } else {
                            body
                        };
                        let _ = tx.send(MatrixEvent::Message {
                            room_id: room.room_id().to_owned(),
                            sender: event.sender.clone(),
                            body,
                            timestamp: event
                                .origin_server_ts
                                .as_secs()
                                .into(),
                            event_id: event.event_id.to_string(),
                            reply_to_event_id,
                        });
                        let _ = tx.send(MatrixEvent::RoomsUpdated);
                    }
                },
            );

            // Register typing indicator handler
            let tx_typing = tx.clone();
            client.add_event_handler(
                move |event: SyncEphemeralRoomEvent<TypingEventContent>, room: Room| {
                    let tx = tx_typing.clone();
                    async move {
                        let _ = tx.send(MatrixEvent::Typing {
                            room_id: room.room_id().to_owned(),
                            user_ids: event.content.user_ids,
                        });
                    }
                },
            );

            // Register presence handler
            let tx_presence = tx.clone();
            client.add_event_handler(
                move |event: PresenceEvent| {
                    let tx = tx_presence.clone();
                    async move {
                        let _ = tx.send(MatrixEvent::Presence {
                            user_id: event.sender.to_string(),
                            presence: format!("{}", event.content.presence),
                        });
                    }
                },
            );

            // Register reaction handler
            let tx_react = tx.clone();
            client.add_event_handler(
                move |event: OriginalSyncReactionEvent, room: Room| {
                    let tx = tx_react.clone();
                    async move {
                        let _ = tx.send(MatrixEvent::Reaction {
                            room_id: room.room_id().to_owned(),
                            event_id: event.content.relates_to.event_id.to_string(),
                            key: event.content.relates_to.key,
                        });
                    }
                },
            );

            // Register incoming verification request handler
            let tx_verify = tx.clone();
            let aid_verify = account_id.clone();
            client.add_event_handler(
                move |event: matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent| {
                    let tx = tx_verify.clone();
                    let aid = aid_verify.clone();
                    async move {
                        let _ = tx.send(MatrixEvent::VerificationIncoming {
                            account_id: aid,
                            user_id: event.sender.to_string(),
                            flow_id: event.content.transaction_id.to_string(),
                        });
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
        use futures_util::StreamExt;

        let joined = self.client.joined_rooms();

        // Collect space room IDs first
        let space_ids: Vec<OwnedRoomId> = joined
            .iter()
            .filter(|r| r.is_space())
            .map(|r| r.room_id().to_owned())
            .collect();

        let mut result = Vec::new();
        for room in &joined {
            let name = room
                .cached_display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| room.room_id().to_string());
            let is_dm = room.is_direct().await.unwrap_or(false);
            let encrypted = room.encryption_state().is_encrypted();
            let is_space = room.is_space();

            // Check if this room belongs to a parent space we've joined
            let parent_space_id = if !is_space {
                match room.parent_spaces().await {
                    Ok(stream) => {
                        let parents: Vec<_> = stream
                            .filter_map(|r| async { r.ok() })
                            .collect()
                            .await;
                        parents.iter().find_map(|p| {
                            let pid = match p {
                                matrix_sdk::room::ParentSpace::Reciprocal(r)
                                | matrix_sdk::room::ParentSpace::WithPowerlevel(r)
                                | matrix_sdk::room::ParentSpace::Illegitimate(r) => r.room_id().to_owned(),
                                matrix_sdk::room::ParentSpace::Unverifiable(id) => id.clone(),
                            };
                            if space_ids.contains(&pid) { Some(pid) } else { None }
                        })
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            result.push(RoomInfo {
                id: room.room_id().to_owned(),
                name,
                is_dm,
                encrypted,
                is_space,
                parent_space_id,
                unread: room.num_unread_notifications().into(),
                account_id: self.user_id.clone(),
            });
        }
        result
    }

    /// Parse a single timeline event into a DisplayMessage (if it's a supported message type)
    pub fn timeline_event_to_display_message(
        ev: &AnySyncTimelineEvent,
    ) -> Option<crate::app::DisplayMessage> {
        match ev {
            AnySyncTimelineEvent::MessageLike(
                AnySyncMessageLikeEvent::RoomMessage(SyncRoomMessageEvent::Original(original)),
            ) => {
                let reply_to_event_id = match &original.content.relates_to {
                    Some(Relation::Reply { in_reply_to }) => {
                        Some(in_reply_to.event_id.to_string())
                    }
                    _ => None,
                };
                if let MessageType::Image(ref img) = original.content.msgtype {
                    Some(crate::app::DisplayMessage {
                        sender: original.sender.to_string(),
                        content: crate::app::MessageContent::Image {
                            body: img.filename().to_string(),
                            source: img.source.clone(),
                            protocol: None,
                            loading: false,
                        },
                        timestamp: original.origin_server_ts.as_secs().into(),
                        event_id: Some(original.event_id.to_string()),
                        reply_to_sender: None,
                        reply_to_body: None,
                        reactions: Vec::new(),
                        reply_to_event_id_raw: reply_to_event_id,
                    })
                } else if let MessageType::File(ref f) = original.content.msgtype {
                    Some(crate::app::DisplayMessage {
                        sender: original.sender.to_string(),
                        content: crate::app::MessageContent::File {
                            body: f.filename().to_string(),
                            source: f.source.clone(),
                            media_type: crate::app::FileKind::File,
                        },
                        timestamp: original.origin_server_ts.as_secs().into(),
                        event_id: Some(original.event_id.to_string()),
                        reply_to_sender: None,
                        reply_to_body: None,
                        reactions: Vec::new(),
                        reply_to_event_id_raw: reply_to_event_id,
                    })
                } else if let MessageType::Video(ref v) = original.content.msgtype {
                    Some(crate::app::DisplayMessage {
                        sender: original.sender.to_string(),
                        content: crate::app::MessageContent::File {
                            body: v.filename().to_string(),
                            source: v.source.clone(),
                            media_type: crate::app::FileKind::Video,
                        },
                        timestamp: original.origin_server_ts.as_secs().into(),
                        event_id: Some(original.event_id.to_string()),
                        reply_to_sender: None,
                        reply_to_body: None,
                        reactions: Vec::new(),
                        reply_to_event_id_raw: reply_to_event_id,
                    })
                } else if let MessageType::Audio(ref a) = original.content.msgtype {
                    Some(crate::app::DisplayMessage {
                        sender: original.sender.to_string(),
                        content: crate::app::MessageContent::File {
                            body: a.filename().to_string(),
                            source: a.source.clone(),
                            media_type: crate::app::FileKind::Audio,
                        },
                        timestamp: original.origin_server_ts.as_secs().into(),
                        event_id: Some(original.event_id.to_string()),
                        reply_to_sender: None,
                        reply_to_body: None,
                        reactions: Vec::new(),
                        reply_to_event_id_raw: reply_to_event_id,
                    })
                } else {
                    let body = match &original.content.msgtype {
                        MessageType::Text(text) => text.body.clone(),
                        MessageType::Notice(n) => n.body.clone(),
                        MessageType::Emote(e) => format!("* {}", e.body),
                        _ => "[unsupported message type]".to_string(),
                    };
                    let body = if reply_to_event_id.is_some() {
                        strip_reply_fallback(&body)
                    } else {
                        body
                    };
                    Some(crate::app::DisplayMessage {
                        sender: original.sender.to_string(),
                        content: crate::app::MessageContent::Text(body),
                        timestamp: original.origin_server_ts.as_secs().into(),
                        event_id: Some(original.event_id.to_string()),
                        reply_to_sender: None,
                        reply_to_body: None,
                        reactions: Vec::new(),
                        reply_to_event_id_raw: reply_to_event_id,
                    })
                }
            }
            _ => None,
        }
    }

    /// Fetch message history with pagination support
    pub async fn fetch_history_paged(
        &self,
        room_id: &OwnedRoomId,
        from: Option<&str>,
        limit: u32,
    ) -> Result<(Vec<crate::app::DisplayMessage>, Option<String>)> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;

        let mut options = MessagesOptions::backward();
        options.limit = UInt::from(limit);
        if let Some(token) = from {
            options = options.from(Some(token));
        } else {
            let prev_batch = room.last_prev_batch();
            info!(
                "fetch_history for {} — prev_batch: {:?}",
                room_id,
                prev_batch.as_deref().unwrap_or("None")
            );
            match prev_batch {
                Some(ref token) => {
                    options = options.from(Some(token.as_str()));
                }
                None => {
                    // No pagination token yet — sync may still be running
                    info!("No prev_batch for {} — returning empty", room_id);
                    return Ok((Vec::new(), None));
                }
            }
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
                Ok(ev) => {
                    if let Some(dm) = Self::timeline_event_to_display_message(&ev) {
                        messages.push(dm);
                    }
                }
                Err(e) => {
                    info!("Failed to deserialize event: {}", e);
                    messages.push(crate::app::DisplayMessage {
                        sender: "".to_string(),
                        content: crate::app::MessageContent::Text("[encrypted message — unable to decrypt]".to_string()),
                        timestamp: 0,
                        event_id: None,
                        reply_to_sender: None,
                        reply_to_body: None,
                        reactions: Vec::new(),
                        reply_to_event_id_raw: None,
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

        Ok((messages, response.end))
    }

    /// Fetch recent message history for a room (convenience wrapper)
    pub async fn fetch_history(
        &self,
        room_id: &OwnedRoomId,
        limit: u32,
    ) -> Result<Vec<crate::app::DisplayMessage>> {
        let (msgs, _) = self.fetch_history_paged(room_id, None, limit).await?;
        Ok(msgs)
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

    /// Edit a message (send a replacement event)
    pub async fn edit_message(
        &self,
        room_id: &OwnedRoomId,
        event_id: &str,
        new_body: &str,
    ) -> Result<()> {
        use matrix_sdk::ruma::OwnedEventId;
        use matrix_sdk::ruma::events::room::message::ReplacementMetadata;

        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let eid: OwnedEventId = event_id.parse()?;
        let content = RoomMessageEventContent::text_plain(new_body)
            .make_replacement(ReplacementMetadata::new(eid, None));
        room.send(content).await?;
        Ok(())
    }

    /// Redact (delete) a message
    pub async fn redact_message(
        &self,
        room_id: &OwnedRoomId,
        event_id: &str,
    ) -> Result<()> {
        use matrix_sdk::ruma::OwnedEventId;

        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let eid: OwnedEventId = event_id.parse()?;
        room.redact(&eid, None, None).await?;
        Ok(())
    }

    /// Send a reply to a message
    pub async fn send_reply(
        &self,
        room_id: &OwnedRoomId,
        body: &str,
        reply_to_event_id: &str,
        reply_to_sender: &str,
    ) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let reply_eid: OwnedEventId = reply_to_event_id.parse()?;
        let reply_uid: OwnedUserId = reply_to_sender.parse()?;
        let metadata = ReplyMetadata::new(&reply_eid, &reply_uid, None);
        let content = RoomMessageEventContentWithoutRelation::text_plain(body)
            .make_reply_to(metadata, ForwardThread::Yes, AddMentions::Yes);
        room.send(content).await?;
        Ok(())
    }

    /// Send a reaction to a message
    pub async fn send_reaction(
        &self,
        room_id: &OwnedRoomId,
        event_id: &str,
        emoji: &str,
    ) -> Result<()> {
        use matrix_sdk::ruma::events::reaction::ReactionEventContent;

        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let eid: OwnedEventId = event_id.parse()?;
        let content = ReactionEventContent::new(Annotation::new(eid, emoji.to_string()));
        room.send(content).await?;
        Ok(())
    }

    /// Download an image from the media server
    /// Download full media content (for saving to disk or inline display)
    pub async fn download_media(&self, source: &MediaSource) -> Result<Vec<u8>> {
        let request = MediaRequestParameters {
            source: source.clone(),
            format: MediaFormat::File,
        };
        Ok(self.client.media().get_media_content(&request, true).await?)
    }

    /// Send a file attachment to a room
    pub async fn send_attachment(
        &self,
        room_id: &OwnedRoomId,
        path: &std::path::Path,
    ) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let data = std::fs::read(path)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let content_type = mime_from_extension(ext);
        let config = matrix_sdk::attachment::AttachmentConfig::new();
        room.send_attachment(filename, &content_type, data, config)
            .await?;
        Ok(())
    }

    /// Send a read receipt for a message
    pub async fn send_read_receipt(
        &self,
        room_id: &OwnedRoomId,
        event_id: &str,
    ) -> Result<()> {
        let room = self
            .client
            .get_room(room_id)
            .ok_or_else(|| anyhow::anyhow!("Room not found"))?;
        let eid: OwnedEventId = event_id.parse()?;
        room.send_single_receipt(
            create_receipt::v3::ReceiptType::Read,
            ReceiptThread::Unthreaded,
            eid,
        )
        .await?;
        Ok(())
    }


    /// Get detailed room info including member list
    pub async fn get_room_details(&self, room_id: &OwnedRoomId) -> Option<RoomDetails> {
        let room = self.client.get_room(room_id)?;
        let name = room
            .cached_display_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| room.room_id().to_string());
        let topic = room.topic();
        let member_count = room.joined_members_count();
        let encryption = if room.encryption_state().is_encrypted() {
            "Encrypted".to_string()
        } else {
            "Not encrypted".to_string()
        };

        // Fetch member list
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(member_list) => member_list
                .into_iter()
                .map(|m| MemberInfo {
                    user_id: m.user_id().to_string(),
                    display_name: m.name().to_string(),
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        Some(RoomDetails {
            name,
            topic,
            member_count,
            encryption,
            room_id: room.room_id().to_string(),
            members,
        })
    }

    /// Get space details including child rooms
    pub async fn get_space_details(&self, room_id: &OwnedRoomId) -> Option<SpaceDetails> {
        let room = self.client.get_room(room_id)?;
        let name = room
            .cached_display_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| room.room_id().to_string());
        let topic = room.topic();
        let member_count = room.joined_members_count();

        // Find child rooms that belong to this space
        let mut children = Vec::new();
        for child_room in self.client.joined_rooms() {
            if child_room.room_id() == room_id {
                continue; // skip self
            }
            // Check if this room has this space as parent
            if let Ok(stream) = child_room.parent_spaces().await {
                use futures_util::StreamExt;
                let parents: Vec<_> = stream
                    .filter_map(|r| async { r.ok() })
                    .collect()
                    .await;
                let is_child = parents.iter().any(|p| {
                    let pid = match p {
                        matrix_sdk::room::ParentSpace::Reciprocal(r)
                        | matrix_sdk::room::ParentSpace::WithPowerlevel(r)
                        | matrix_sdk::room::ParentSpace::Illegitimate(r) => r.room_id().to_owned(),
                        matrix_sdk::room::ParentSpace::Unverifiable(id) => id.clone(),
                    };
                    &pid == room_id
                });
                if is_child {
                    let child_name = child_room
                        .cached_display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| child_room.room_id().to_string());
                    children.push(SpaceChildInfo {
                        name: child_name,
                        is_space: child_room.is_space(),
                        encrypted: child_room.encryption_state().is_encrypted(),
                        member_count: child_room.joined_members_count(),
                    });
                }
            }
        }

        Some(SpaceDetails {
            name,
            topic,
            member_count,
            room_id: room.room_id().to_string(),
            rooms: children,
        })
    }

    /// Check if this session's cross-signing is verified
    pub async fn is_session_verified(&self) -> bool {
        let status = self.client.encryption().cross_signing_status().await;
        match status {
            Some(s) => s.is_complete(),
            None => false,
        }
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


    /// Request self-verification (sends request to all other devices)
    pub async fn request_self_verification(
        &self,
        tx: mpsc::UnboundedSender<MatrixEvent>,
    ) -> Result<()> {
        let user_id: &UserId = self.client.user_id()
            .ok_or_else(|| anyhow::anyhow!("Not logged in"))?;
        let identity = self.client.encryption()
            .get_user_identity(user_id).await?
            .ok_or_else(|| anyhow::anyhow!("Own identity not found"))?;

        let methods = vec![VerificationMethod::SasV1];
        let request = identity.request_verification_with_methods(methods).await?;
        let flow_id = request.flow_id().to_string();
        info!("Sent self-verification request, flow_id={}", flow_id);

        // Spawn a task to watch the request state transitions
        Self::spawn_verification_request_watcher(request, tx, flow_id);
        Ok(())
    }

    /// Get a pending VerificationRequest by user_id and flow_id
    pub async fn get_verification_request(
        &self,
        user_id_str: &str,
        flow_id: &str,
    ) -> Option<VerificationRequest> {
        let user_id = OwnedUserId::try_from(user_id_str).ok()?;
        self.client.encryption()
            .get_verification_request(&user_id, flow_id).await
    }

    /// Accept an incoming verification request and start SAS
    pub async fn accept_and_start_sas(
        &self,
        user_id_str: &str,
        flow_id: &str,
        tx: mpsc::UnboundedSender<MatrixEvent>,
    ) -> Result<SasVerification> {
        let request = self.get_verification_request(user_id_str, flow_id).await
            .ok_or_else(|| anyhow::anyhow!("Verification request not found"))?;

        request.accept().await?;
        info!("Accepted verification request, checking state...");

        // Check if already ready (accept() may transition immediately)
        let current = request.state();
        info!("Verification request state after accept: {:?}", &current);
        let already_ready = matches!(current, VerificationRequestState::Ready { .. });

        if already_ready {
            info!("Request already ready after accept, starting SAS");
            let sas = request.start_sas().await?
                .ok_or_else(|| anyhow::anyhow!("Failed to start SAS verification"))?;
            // We started SAS so we're the initiator — don't call accept
            let flow_id = flow_id.to_string();
            Self::spawn_sas_watcher(sas.clone(), tx, flow_id);
            return Ok(sas);
        }

        if let VerificationRequestState::Transitioned { verification } = current {
            if let Some(sas) = verification.sas() {
                info!("Request already transitioned to SAS after accept");
                // Other side started SAS — we need to accept
                sas.accept().await?;
                let flow_id = flow_id.to_string();
                Self::spawn_sas_watcher(sas.clone(), tx, flow_id);
                return Ok(sas);
            }
        }

        // Not ready yet — subscribe before waiting so we don't miss transitions
        let mut changes = request.changes();
        while let Some(state) = changes.next().await {
            info!("Verification request state change: {:?}", &state);
            match state {
                VerificationRequestState::Ready { .. } => {
                    let sas = request.start_sas().await?
                        .ok_or_else(|| anyhow::anyhow!("Failed to start SAS verification"))?;
                    // We started SAS — don't accept
                    let flow_id = flow_id.to_string();
                    Self::spawn_sas_watcher(sas.clone(), tx, flow_id);
                    return Ok(sas);
                }
                VerificationRequestState::Transitioned { verification } => {
                    if let Some(sas) = verification.sas() {
                        // Other side started SAS — accept
                        sas.accept().await?;
                        let flow_id = flow_id.to_string();
                        Self::spawn_sas_watcher(sas.clone(), tx, flow_id);
                        return Ok(sas);
                    }
                }
                VerificationRequestState::Done
                | VerificationRequestState::Cancelled(_) => {
                    return Err(anyhow::anyhow!("Request cancelled before SAS could start"));
                }
                _ => {}
            }
        }

        Err(anyhow::anyhow!("Verification request stream ended unexpectedly"))
    }

    /// Spawn a background task watching VerificationRequest state changes
    fn spawn_verification_request_watcher(
        request: VerificationRequest,
        tx: mpsc::UnboundedSender<MatrixEvent>,
        flow_id: String,
    ) {
        tokio::spawn(async move {
            // Check current state first — the request may already be ready
            // before we start listening to the stream
            let current = request.state();
            info!("Verification request initial state: {:?}", &current);
            match current {
                VerificationRequestState::Ready { .. } => {
                    info!("Request already ready, starting SAS immediately");
                    match request.start_sas().await {
                        Ok(Some(sas)) => {
                            sas.accept().await.ok();
                            let _ = tx.send(MatrixEvent::SasStarted {
                                flow_id: flow_id.clone(),
                                sas: sas.clone(),
                            });
                            Self::spawn_sas_watcher(sas, tx, flow_id);
                        }
                        Ok(None) => {
                            let _ = tx.send(MatrixEvent::SasCancelled {
                                flow_id,
                                reason: "Failed to start SAS".to_string(),
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(MatrixEvent::SasCancelled {
                                flow_id,
                                reason: e.to_string(),
                            });
                        }
                    }
                    return;
                }
                VerificationRequestState::Done | VerificationRequestState::Cancelled(_) => {
                    let _ = tx.send(MatrixEvent::SasCancelled {
                        flow_id,
                        reason: "Request already finished".to_string(),
                    });
                    return;
                }
                _ => {} // Not ready yet, fall through to stream
            }

            let mut changes = request.changes();
            while let Some(state) = changes.next().await {
                info!("Verification request state change: {:?}", &state);
                match state {
                    VerificationRequestState::Ready { .. } => {
                        // Other device accepted — start the SAS flow
                        info!("Verification request ready, starting SAS");
                        match request.start_sas().await {
                            Ok(Some(sas)) => {
                                sas.accept().await.ok();
                                let _ = tx.send(MatrixEvent::SasStarted {
                                    flow_id: flow_id.clone(),
                                    sas: sas.clone(),
                                });
                                let fid = flow_id.clone();
                                Self::spawn_sas_watcher(sas, tx.clone(), fid);
                                break;
                            }
                            Ok(None) => {
                                let _ = tx.send(MatrixEvent::SasCancelled {
                                    flow_id: flow_id.clone(),
                                    reason: "Failed to start SAS".to_string(),
                                });
                                break;
                            }
                            Err(e) => {
                                let _ = tx.send(MatrixEvent::SasCancelled {
                                    flow_id: flow_id.clone(),
                                    reason: e.to_string(),
                                });
                                break;
                            }
                        }
                    }
                    VerificationRequestState::Transitioned { verification } => {
                        // Other side started SAS directly
                        if let Some(sas) = verification.sas() {
                            sas.accept().await.ok();
                            let _ = tx.send(MatrixEvent::SasStarted {
                                flow_id: flow_id.clone(),
                                sas: sas.clone(),
                            });
                            let fid = flow_id.clone();
                            Self::spawn_sas_watcher(sas, tx.clone(), fid);
                        }
                        break;
                    }
                    VerificationRequestState::Done => break,
                    VerificationRequestState::Cancelled(info) => {
                        let _ = tx.send(MatrixEvent::SasCancelled {
                            flow_id: flow_id.clone(),
                            reason: info.reason().to_string(),
                        });
                        break;
                    }
                    _ => {}
                }
            }
        });
    }

    /// Spawn a background task watching SAS verification state changes
    fn spawn_sas_watcher(
        sas: SasVerification,
        tx: mpsc::UnboundedSender<MatrixEvent>,
        flow_id: String,
    ) {
        // Subscribe to changes BEFORE spawning so we don't miss any transitions
        let mut changes = sas.changes();

        tokio::spawn(async move {
            use matrix_sdk::encryption::verification::SasState;

            // Check if emojis are already available (keys may have exchanged before we got here)
            if sas.can_be_presented() {
                info!("SAS emojis already available at watcher start");
                if let Some(emojis) = sas.emoji() {
                    let emoji_pairs: Vec<(String, String)> = emojis.iter()
                        .map(|e| (e.symbol.to_string(), e.description.to_string()))
                        .collect();
                    let _ = tx.send(MatrixEvent::SasEmojis {
                        flow_id: flow_id.clone(),
                        emojis: emoji_pairs,
                    });
                }
            }

            while let Some(state) = changes.next().await {
                info!("SAS state change: {:?}", &state);
                match state {
                    SasState::KeysExchanged { emojis, .. } => {
                        if let Some(emoji_sas) = emojis {
                            let emoji_pairs: Vec<(String, String)> = emoji_sas.emojis.iter()
                                .map(|e| (e.symbol.to_string(), e.description.to_string()))
                                .collect();
                            let _ = tx.send(MatrixEvent::SasEmojis {
                                flow_id: flow_id.clone(),
                                emojis: emoji_pairs,
                            });
                        } else {
                            // Keys exchanged but no emojis — try polling
                            info!("KeysExchanged without emojis, polling sas.emoji()");
                            if let Some(emojis) = sas.emoji() {
                                let emoji_pairs: Vec<(String, String)> = emojis.iter()
                                    .map(|e| (e.symbol.to_string(), e.description.to_string()))
                                    .collect();
                                let _ = tx.send(MatrixEvent::SasEmojis {
                                    flow_id: flow_id.clone(),
                                    emojis: emoji_pairs,
                                });
                            }
                        }
                    }
                    SasState::Done { .. } => {
                        let _ = tx.send(MatrixEvent::SasDone {
                            flow_id: flow_id.clone(),
                        });
                        break;
                    }
                    SasState::Cancelled(info) => {
                        let _ = tx.send(MatrixEvent::SasCancelled {
                            flow_id: flow_id.clone(),
                            reason: info.reason().to_string(),
                        });
                        break;
                    }
                    _ => {
                        // Also check emojis on any other state change
                        if sas.can_be_presented() {
                            if let Some(emojis) = sas.emoji() {
                                let emoji_pairs: Vec<(String, String)> = emojis.iter()
                                    .map(|e| (e.symbol.to_string(), e.description.to_string()))
                                    .collect();
                                let _ = tx.send(MatrixEvent::SasEmojis {
                                    flow_id: flow_id.clone(),
                                    emojis: emoji_pairs,
                                });
                            }
                        }
                    }
                }
            }
        });
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


fn e2ee_settings() -> EncryptionSettings {
    EncryptionSettings {
        backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
        auto_enable_backups: true,
        ..Default::default()
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
