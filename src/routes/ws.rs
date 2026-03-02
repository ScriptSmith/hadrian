//! WebSocket handler for real-time event subscriptions.
//!
//! This module provides WebSocket endpoints for subscribing to server events
//! such as audit logs, usage tracking, circuit breaker state changes, etc.
//!
//! # Authentication
//!
//! WebSocket connections can be authenticated via:
//! - Query parameter `token` - API key for programmatic access
//! - Session cookie - For browser-based access (requires prior OIDC login)
//!
//! # Subscription Protocol
//!
//! After connecting, clients can send JSON messages to subscribe to topics:
//!
//! ```json
//! {"type": "subscribe", "topics": ["audit", "usage", "health"]}
//! ```
//!
//! Available topics:
//! - `audit` - Audit log events
//! - `usage` - Usage tracking events
//! - `health` - Provider health and circuit breaker events
//! - `budget` - Budget threshold events
//! - `rate_limit` - Rate limit warning events
//! - `all` - All events (wildcard)
//!
//! To unsubscribe:
//! ```json
//! {"type": "unsubscribe", "topics": ["usage"]}
//! ```
//!
//! # Keepalive
//!
//! The server sends ping frames every 30 seconds. Clients should respond with pong.
//! Connections that don't respond within 60 seconds are terminated.

use std::{collections::HashSet, time::Duration};

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tower_cookies::Cookies;
#[cfg(feature = "sso")]
use uuid::Uuid;

use crate::{
    AppState,
    auth::{AuthError, Identity},
    cache::CacheKeys,
    config::WebSocketConfig,
    events::{EventTopic, ServerEvent},
    models::{CachedApiKey, has_valid_prefix, hash_api_key},
};

/// Query parameters for WebSocket connection.
#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    /// API key token for authentication.
    pub token: Option<String>,
    /// Initial topics to subscribe to (comma-separated).
    pub topics: Option<String>,
}

/// Client-to-server WebSocket messages.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to one or more topics.
    Subscribe { topics: Vec<String> },
    /// Unsubscribe from one or more topics.
    Unsubscribe { topics: Vec<String> },
    /// Ping message (client-initiated keepalive).
    Ping,
}

/// Server-to-client WebSocket messages.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)] // Error variant is part of the protocol but may not be used yet
pub enum ServerMessage {
    /// Connection established successfully.
    Connected {
        /// Authenticated user's external ID (if authenticated).
        user_id: Option<String>,
        /// Currently subscribed topics.
        subscribed_topics: Vec<String>,
    },
    /// Subscription confirmed.
    Subscribed {
        /// Topics that were successfully subscribed.
        topics: Vec<String>,
    },
    /// Unsubscription confirmed.
    Unsubscribed {
        /// Topics that were successfully unsubscribed.
        topics: Vec<String>,
    },
    /// Server event broadcast.
    Event {
        /// The event topic.
        topic: EventTopic,
        /// The event payload.
        #[serde(flatten)]
        event: ServerEvent,
    },
    /// Error message.
    Error {
        /// Error code.
        code: String,
        /// Human-readable error message.
        message: String,
    },
    /// Pong response to client ping.
    Pong,
}

/// WebSocket upgrade handler.
///
/// Handles the HTTP upgrade to WebSocket and initiates the connection.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WsQueryParams>,
    cookies: Cookies,
) -> Result<Response, AuthError> {
    let ws_config = state.config.features.websocket.clone();

    // Authenticate the connection
    let identity = authenticate_ws(&params, Some(&cookies), &state, &ws_config).await?;

    // Parse initial topics from query params
    let initial_topics: HashSet<EventTopic> = params
        .topics
        .as_deref()
        .map(|t| t.split(',').filter_map(|s| parse_topic(s.trim())).collect())
        .unwrap_or_default();

    tracing::info!(
        external_id = ?identity.as_ref().map(|i| &i.external_id),
        initial_topics = ?initial_topics,
        "WebSocket connection upgrading"
    );

    // Upgrade to WebSocket
    Ok(ws.on_upgrade(move |socket| {
        handle_socket(socket, state, identity, initial_topics, ws_config)
    }))
}

/// Authenticate a WebSocket connection.
async fn authenticate_ws(
    params: &WsQueryParams,
    cookies: Option<&Cookies>,
    state: &AppState,
    ws_config: &WebSocketConfig,
) -> Result<Option<Identity>, AuthError> {
    #[cfg(not(feature = "sso"))]
    let _ = &cookies;
    // Try API key authentication first
    if let Some(token) = &params.token {
        return authenticate_with_api_key(token, state).await;
    }

    // Try session cookie authentication (SSO only)
    #[cfg(feature = "sso")]
    if let Some(identity) = try_session_auth(cookies, state).await? {
        return Ok(Some(identity));
    }

    // Check if authentication is required
    if ws_config.require_auth {
        return Err(AuthError::MissingCredentials);
    }

    // Allow unauthenticated connections if require_auth is false
    // (for development or when auth is handled externally)
    if !state.config.auth.is_auth_enabled() {
        return Ok(None);
    }

    // If auth is configured but require_auth is false, allow unauthenticated connections
    Ok(None)
}

/// Authenticate using an API key.
async fn authenticate_with_api_key(
    token: &str,
    state: &AppState,
) -> Result<Option<Identity>, AuthError> {
    // Get key prefix from config
    let key_prefix = state.config.auth.api_key_config().key_prefix.as_str();

    // Validate key prefix
    if !has_valid_prefix(token, key_prefix) {
        return Err(AuthError::InvalidApiKeyFormat);
    }

    let key_hash = hash_api_key(token);

    // Check cache first
    if let Some(cache) = &state.cache {
        let cache_key = CacheKeys::api_key(&key_hash);
        if let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await
            && let Ok(cached) = serde_json::from_slice::<CachedApiKey>(&bytes)
        {
            // Check revocation
            if cached.key.revoked_at.is_some() {
                return Err(AuthError::InvalidApiKey);
            }

            // Check expiration
            if let Some(expires_at) = cached.key.expires_at
                && expires_at < chrono::Utc::now()
            {
                return Err(AuthError::ExpiredApiKey);
            }

            return Ok(Some(Identity {
                external_id: format!("api_key:{}", cached.key.id),
                email: None,
                name: Some(cached.key.name.clone()),
                user_id: cached.user_id,
                roles: Vec::new(),
                idp_groups: Vec::new(),
                org_ids: cached.org_id.map(|id| id.to_string()).into_iter().collect(),
                team_ids: cached
                    .team_id
                    .map(|id| id.to_string())
                    .into_iter()
                    .collect(),
                project_ids: cached
                    .project_id
                    .map(|id| id.to_string())
                    .into_iter()
                    .collect(),
            }));
        }
    }

    // Fall back to database lookup
    if let Some(db) = &state.db
        && let Some(key_with_owner) = db
            .api_keys()
            .get_by_hash(&key_hash)
            .await
            .map_err(|e| AuthError::Internal(e.to_string()))?
    {
        // Check if key is revoked
        if key_with_owner.key.revoked_at.is_some() {
            return Err(AuthError::InvalidApiKey);
        }

        // Check expiration
        if let Some(expires_at) = key_with_owner.key.expires_at
            && expires_at < chrono::Utc::now()
        {
            return Err(AuthError::ExpiredApiKey);
        }

        return Ok(Some(Identity {
            external_id: format!("api_key:{}", key_with_owner.key.id),
            email: None,
            name: Some(key_with_owner.key.name.clone()),
            user_id: key_with_owner.user_id,
            roles: Vec::new(),
            idp_groups: Vec::new(),
            org_ids: key_with_owner
                .org_id
                .map(|id| id.to_string())
                .into_iter()
                .collect(),
            team_ids: key_with_owner
                .team_id
                .map(|id| id.to_string())
                .into_iter()
                .collect(),
            project_ids: key_with_owner
                .project_id
                .map(|id| id.to_string())
                .into_iter()
                .collect(),
        }));
    }

    Err(AuthError::InvalidApiKey)
}

/// Try to authenticate via session cookie.
#[cfg(feature = "sso")]
async fn try_session_auth(
    cookies: Option<&Cookies>,
    state: &AppState,
) -> Result<Option<Identity>, AuthError> {
    let registry = match &state.oidc_registry {
        Some(registry) => registry,
        None => return Ok(None),
    };

    let session_config = match state.config.auth.session_config() {
        Some(config) => config,
        None => return Ok(None),
    };

    let cookies = match cookies {
        Some(c) => c,
        None => return Ok(None),
    };

    let session_cookie = match cookies.get(&session_config.cookie_name) {
        Some(c) => c,
        None => return Ok(None),
    };

    let session_id: Uuid = session_cookie
        .value()
        .parse()
        .map_err(|_| AuthError::InvalidToken)?;

    let session = crate::auth::session_store::validate_and_refresh_session(
        registry.session_store().as_ref(),
        session_id,
        &session_config.enhanced,
    )
    .await
    .map_err(|e| match e {
        crate::auth::session_store::SessionError::NotFound => AuthError::SessionNotFound,
        crate::auth::session_store::SessionError::Expired => AuthError::SessionExpired,
        _ => AuthError::Internal(format!("Session error: {}", e)),
    })?;

    // Look up internal user ID
    let user_id = if let Some(db) = &state.db {
        db.users()
            .get_by_external_id(&session.external_id)
            .await
            .map_err(|e| AuthError::Internal(e.to_string()))?
            .map(|u| u.id)
    } else {
        None
    };

    // Use session.roles for role names; fall back to groups for backwards compatibility
    let roles = if session.roles.is_empty() {
        session.groups.clone()
    } else {
        session.roles.clone()
    };

    Ok(Some(Identity {
        external_id: session.external_id,
        email: session.email,
        name: session.name,
        user_id,
        roles,
        idp_groups: session.groups.clone(),
        org_ids: Vec::new(),
        team_ids: Vec::new(),
        project_ids: Vec::new(),
    }))
}

/// Parse a topic string into an EventTopic.
fn parse_topic(s: &str) -> Option<EventTopic> {
    match s.to_lowercase().as_str() {
        "audit" => Some(EventTopic::Audit),
        "usage" => Some(EventTopic::Usage),
        "health" => Some(EventTopic::Health),
        "budget" => Some(EventTopic::Budget),
        "rate_limit" | "ratelimit" => Some(EventTopic::RateLimit),
        "all" | "*" => Some(EventTopic::All),
        _ => None,
    }
}

/// Handle an established WebSocket connection.
async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    identity: Option<Identity>,
    initial_topics: HashSet<EventTopic>,
    ws_config: WebSocketConfig,
) {
    let (sender, receiver) = socket.split();

    // Subscribe to the event bus
    let event_rx = state.event_bus.subscribe();

    // Initialize subscribed topics
    let subscribed_topics = if initial_topics.is_empty() {
        // Default to all topics if none specified
        let mut topics = HashSet::new();
        topics.insert(EventTopic::All);
        topics
    } else {
        initial_topics
    };

    // Create connection state with configuration
    let conn = WsConnection {
        sender,
        event_rx,
        subscribed_topics,
        identity,
        ping_interval: Duration::from_secs(ws_config.ping_interval_secs),
        pong_timeout: Duration::from_secs(ws_config.pong_timeout_secs),
    };

    // Run the connection handler
    if let Err(e) = conn.run(receiver).await {
        tracing::debug!(error = %e, "WebSocket connection closed");
    }
}

/// WebSocket connection state.
struct WsConnection {
    sender: SplitSink<WebSocket, Message>,
    event_rx: broadcast::Receiver<ServerEvent>,
    subscribed_topics: HashSet<EventTopic>,
    identity: Option<Identity>,
    ping_interval: Duration,
    pong_timeout: Duration,
}

impl WsConnection {
    /// Run the WebSocket connection loop.
    async fn run(
        mut self,
        mut receiver: futures_util::stream::SplitStream<WebSocket>,
    ) -> Result<(), WsError> {
        // Send connected message
        let connected_msg = ServerMessage::Connected {
            user_id: self.identity.as_ref().map(|i| i.external_id.clone()),
            subscribed_topics: self
                .subscribed_topics
                .iter()
                .map(|t| format!("{:?}", t).to_lowercase())
                .collect(),
        };
        self.send_message(&connected_msg).await?;

        // Set up ping interval using configured values
        let mut ping_interval = tokio::time::interval(self.ping_interval);
        let mut last_pong = std::time::Instant::now();

        loop {
            tokio::select! {
                // Handle incoming client messages
                msg = receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_client_message(&text).await {
                                tracing::debug!(error = %e, "Error handling client message");
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            last_pong = std::time::Instant::now();
                        }
                        Some(Ok(Message::Close(_))) => {
                            tracing::debug!("Client initiated close");
                            break;
                        }
                        Some(Err(e)) => {
                            tracing::debug!(error = %e, "WebSocket receive error");
                            break;
                        }
                        None => {
                            tracing::debug!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }

                // Handle events from the event bus
                event = self.event_rx.recv() => {
                    match event {
                        Ok(event) => {
                            if self.should_forward_event(&event) {
                                let msg = ServerMessage::Event {
                                    topic: event.topic(),
                                    event,
                                };
                                if let Err(e) = self.send_message(&msg).await {
                                    tracing::debug!(error = %e, "Error sending event");
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            tracing::warn!(count, "WebSocket subscriber lagged, missed events");
                            // Continue processing - we just missed some events
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::debug!("Event bus closed");
                            break;
                        }
                    }
                }

                // Send ping for keepalive
                _ = ping_interval.tick() => {
                    // Check if we've received a pong recently
                    if last_pong.elapsed() > self.pong_timeout {
                        tracing::debug!("Pong timeout, closing connection");
                        break;
                    }

                    if let Err(e) = self.sender.send(Message::Ping(bytes::Bytes::new())).await {
                        tracing::debug!(error = %e, "Error sending ping");
                        break;
                    }
                }
            }
        }

        // Try to send close frame
        let _ = self.sender.close().await;
        Ok(())
    }

    /// Handle a client message.
    async fn handle_client_message(&mut self, text: &str) -> Result<(), WsError> {
        let msg: ClientMessage = serde_json::from_str(text).map_err(WsError::InvalidMessage)?;

        match msg {
            ClientMessage::Subscribe { topics } => {
                let parsed_topics: Vec<EventTopic> =
                    topics.iter().filter_map(|t| parse_topic(t)).collect();

                for topic in &parsed_topics {
                    self.subscribed_topics.insert(*topic);
                }

                let response = ServerMessage::Subscribed {
                    topics: parsed_topics
                        .iter()
                        .map(|t| format!("{:?}", t).to_lowercase())
                        .collect(),
                };
                self.send_message(&response).await?;
            }
            ClientMessage::Unsubscribe { topics } => {
                let parsed_topics: Vec<EventTopic> =
                    topics.iter().filter_map(|t| parse_topic(t)).collect();

                for topic in &parsed_topics {
                    self.subscribed_topics.remove(topic);
                }

                let response = ServerMessage::Unsubscribed {
                    topics: parsed_topics
                        .iter()
                        .map(|t| format!("{:?}", t).to_lowercase())
                        .collect(),
                };
                self.send_message(&response).await?;
            }
            ClientMessage::Ping => {
                self.send_message(&ServerMessage::Pong).await?;
            }
        }

        Ok(())
    }

    /// Check if an event should be forwarded to this client.
    fn should_forward_event(&self, event: &ServerEvent) -> bool {
        let event_topic = event.topic();

        for subscribed in &self.subscribed_topics {
            if subscribed.matches(&event_topic) {
                return true;
            }
        }

        false
    }

    /// Send a message to the client.
    async fn send_message(&mut self, msg: &ServerMessage) -> Result<(), WsError> {
        let json = serde_json::to_string(msg).map_err(WsError::Serialization)?;
        self.sender
            .send(Message::Text(json.into()))
            .await
            .map_err(WsError::Send)?;
        Ok(())
    }
}

/// WebSocket error type.
#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("Invalid message: {0}")]
    InvalidMessage(#[from] serde_json::Error),

    #[error("Send error: {0}")]
    Send(#[from] axum::Error),

    #[error("Serialization error: {0}")]
    Serialization(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_parse_topic() {
        assert_eq!(parse_topic("audit"), Some(EventTopic::Audit));
        assert_eq!(parse_topic("AUDIT"), Some(EventTopic::Audit));
        assert_eq!(parse_topic("usage"), Some(EventTopic::Usage));
        assert_eq!(parse_topic("health"), Some(EventTopic::Health));
        assert_eq!(parse_topic("budget"), Some(EventTopic::Budget));
        assert_eq!(parse_topic("rate_limit"), Some(EventTopic::RateLimit));
        assert_eq!(parse_topic("ratelimit"), Some(EventTopic::RateLimit));
        assert_eq!(parse_topic("all"), Some(EventTopic::All));
        assert_eq!(parse_topic("*"), Some(EventTopic::All));
        assert_eq!(parse_topic("invalid"), None);
    }

    #[test]
    fn test_client_message_deserialize() {
        let subscribe = r#"{"type": "subscribe", "topics": ["audit", "usage"]}"#;
        let msg: ClientMessage = serde_json::from_str(subscribe).unwrap();
        assert!(matches!(msg, ClientMessage::Subscribe { topics } if topics.len() == 2));

        let unsubscribe = r#"{"type": "unsubscribe", "topics": ["audit"]}"#;
        let msg: ClientMessage = serde_json::from_str(unsubscribe).unwrap();
        assert!(matches!(msg, ClientMessage::Unsubscribe { topics } if topics.len() == 1));

        let ping = r#"{"type": "ping"}"#;
        let msg: ClientMessage = serde_json::from_str(ping).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn test_server_message_serialize() {
        let connected = ServerMessage::Connected {
            user_id: Some("user123".to_string()),
            subscribed_topics: vec!["audit".to_string(), "usage".to_string()],
        };
        let json = serde_json::to_string(&connected).unwrap();
        assert!(json.contains("\"type\":\"connected\""));
        assert!(json.contains("\"user_id\":\"user123\""));

        let subscribed = ServerMessage::Subscribed {
            topics: vec!["health".to_string()],
        };
        let json = serde_json::to_string(&subscribed).unwrap();
        assert!(json.contains("\"type\":\"subscribed\""));

        let error = ServerMessage::Error {
            code: "invalid_topic".to_string(),
            message: "Unknown topic".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"code\":\"invalid_topic\""));
    }

    #[test]
    fn test_topic_matching() {
        let mut subscribed = HashSet::new();
        subscribed.insert(EventTopic::Audit);
        subscribed.insert(EventTopic::Usage);

        // Direct match
        let event = ServerEvent::AuditLogCreated {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action: "create".to_string(),
            resource_type: "user".to_string(),
            resource_id: None,
            actor_type: "system".to_string(),
            actor_id: None,
            org_id: None,
            project_id: None,
        };
        assert!(subscribed.iter().any(|t| t.matches(&event.topic())));

        // Non-matching topic
        let event = ServerEvent::BudgetThresholdReached {
            timestamp: chrono::Utc::now(),
            budget_type: crate::events::BudgetType::Daily,
            threshold_percent: 80,
            current_amount_microcents: 8000,
            limit_microcents: 10000,
            user_id: None,
            org_id: None,
            project_id: None,
        };
        assert!(!subscribed.iter().any(|t| t.matches(&event.topic())));

        // All topic matches everything
        let mut subscribed_all = HashSet::new();
        subscribed_all.insert(EventTopic::All);
        assert!(subscribed_all.iter().any(|t| t.matches(&event.topic())));
    }

    #[test]
    fn test_parse_topic_case_insensitive() {
        // Test various case combinations
        assert_eq!(parse_topic("AUDIT"), Some(EventTopic::Audit));
        assert_eq!(parse_topic("Audit"), Some(EventTopic::Audit));
        assert_eq!(parse_topic("AuDiT"), Some(EventTopic::Audit));
        assert_eq!(parse_topic("USAGE"), Some(EventTopic::Usage));
        assert_eq!(parse_topic("HEALTH"), Some(EventTopic::Health));
        assert_eq!(parse_topic("BUDGET"), Some(EventTopic::Budget));
        assert_eq!(parse_topic("RATE_LIMIT"), Some(EventTopic::RateLimit));
        assert_eq!(parse_topic("RATELIMIT"), Some(EventTopic::RateLimit));
        assert_eq!(parse_topic("ALL"), Some(EventTopic::All));
    }

    #[test]
    fn test_parse_topic_all_aliases() {
        // Test both "all" and "*" parse to EventTopic::All
        assert_eq!(parse_topic("all"), Some(EventTopic::All));
        assert_eq!(parse_topic("*"), Some(EventTopic::All));
    }

    #[test]
    fn test_parse_topic_rate_limit_aliases() {
        // Test both snake_case and no-underscore versions
        assert_eq!(parse_topic("rate_limit"), Some(EventTopic::RateLimit));
        assert_eq!(parse_topic("ratelimit"), Some(EventTopic::RateLimit));
    }

    #[test]
    fn test_client_message_subscribe_empty() {
        let msg = r#"{"type": "subscribe", "topics": []}"#;
        let parsed: ClientMessage = serde_json::from_str(msg).unwrap();
        assert!(matches!(parsed, ClientMessage::Subscribe { topics } if topics.is_empty()));
    }

    #[test]
    fn test_client_message_invalid_type() {
        let msg = r#"{"type": "invalid_type", "topics": []}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_message_event_serialization() {
        let event = ServerEvent::UsageRecorded {
            request_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cost_microcents: Some(1000),
            user_id: None,
            org_id: None,
            project_id: None,
            team_id: None,
            service_account_id: None,
        };

        let msg = ServerMessage::Event {
            topic: EventTopic::Usage,
            event,
        };

        let json = serde_json::to_string(&msg).unwrap();
        // The event is flattened into the message with both type fields:
        // - "type": "event" from ServerMessage
        // - "event_type": "usage_recorded" from ServerEvent (uses separate tag to avoid collision)
        assert!(json.contains("\"type\":\"event\""));
        assert!(json.contains("\"event_type\":\"usage_recorded\""));
        assert!(json.contains("\"topic\":\"usage\""));
        assert!(json.contains("\"model\":\"gpt-4\""));
    }

    #[test]
    fn test_server_message_pong() {
        let msg = ServerMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"pong"}"#);
    }

    #[test]
    fn test_server_message_unsubscribed() {
        let msg = ServerMessage::Unsubscribed {
            topics: vec!["audit".to_string(), "usage".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"unsubscribed\""));
        assert!(json.contains("\"topics\":[\"audit\",\"usage\"]"));
    }

    #[test]
    fn test_ws_error_display() {
        // Test InvalidMessage error display
        let parse_err = serde_json::from_str::<ClientMessage>("invalid").unwrap_err();
        let err = WsError::InvalidMessage(parse_err);
        let display = format!("{}", err);
        assert!(display.contains("Invalid message"));

        // Test that WsError implements std::error::Error
        let err: &dyn std::error::Error =
            &WsError::InvalidMessage(serde_json::from_str::<ClientMessage>("{}").unwrap_err());
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_topic_matching_comprehensive() {
        // Test all topic combinations for matching
        let topics = [
            EventTopic::Audit,
            EventTopic::Usage,
            EventTopic::Health,
            EventTopic::Budget,
            EventTopic::RateLimit,
        ];

        // Each topic should only match itself and All
        for topic in &topics {
            let mut subscribed = HashSet::new();
            subscribed.insert(*topic);

            // Should match events of the same topic
            for other_topic in &topics {
                let matches = subscribed.iter().any(|t| t.matches(other_topic));
                if topic == other_topic {
                    assert!(matches, "{:?} should match {:?}", topic, other_topic);
                } else {
                    assert!(!matches, "{:?} should not match {:?}", topic, other_topic);
                }
            }
        }

        // All should match everything
        let mut subscribed_all = HashSet::new();
        subscribed_all.insert(EventTopic::All);
        for topic in &topics {
            assert!(
                subscribed_all.iter().any(|t| t.matches(topic)),
                "All should match {:?}",
                topic
            );
        }
    }

    #[test]
    fn test_ws_query_params_deserialize() {
        // Test with both token and topics
        let json = r#"{"token": "abc123", "topics": "audit,usage"}"#;
        let params: WsQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.token, Some("abc123".to_string()));
        assert_eq!(params.topics, Some("audit,usage".to_string()));

        // Test with only token
        let json = r#"{"token": "abc123"}"#;
        let params: WsQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.token, Some("abc123".to_string()));
        assert!(params.topics.is_none());

        // Test with only topics
        let json = r#"{"topics": "all"}"#;
        let params: WsQueryParams = serde_json::from_str(json).unwrap();
        assert!(params.token.is_none());
        assert_eq!(params.topics, Some("all".to_string()));

        // Test empty params
        let json = r#"{}"#;
        let params: WsQueryParams = serde_json::from_str(json).unwrap();
        assert!(params.token.is_none());
        assert!(params.topics.is_none());
    }

    #[test]
    fn test_initial_topics_parsing() {
        // Simulate parsing initial topics from query params
        let topics_str = "audit,usage,health";
        let parsed: HashSet<EventTopic> = topics_str
            .split(',')
            .filter_map(|s| parse_topic(s.trim()))
            .collect();

        assert_eq!(parsed.len(), 3);
        assert!(parsed.contains(&EventTopic::Audit));
        assert!(parsed.contains(&EventTopic::Usage));
        assert!(parsed.contains(&EventTopic::Health));
    }

    #[test]
    fn test_initial_topics_with_whitespace() {
        // Topics with whitespace should be trimmed
        let topics_str = " audit , usage , health ";
        let parsed: HashSet<EventTopic> = topics_str
            .split(',')
            .filter_map(|s| parse_topic(s.trim()))
            .collect();

        assert_eq!(parsed.len(), 3);
        assert!(parsed.contains(&EventTopic::Audit));
    }

    #[test]
    fn test_initial_topics_with_invalid() {
        // Invalid topics should be filtered out
        let topics_str = "audit,invalid,usage,unknown";
        let parsed: HashSet<EventTopic> = topics_str
            .split(',')
            .filter_map(|s| parse_topic(s.trim()))
            .collect();

        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains(&EventTopic::Audit));
        assert!(parsed.contains(&EventTopic::Usage));
    }
}
