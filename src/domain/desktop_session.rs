//! Desktop device registration and session issuance.
//!
//! These commands wrap wasi-auth sessions + `oauth_device_codes`. They never
//! persist PIN, TOTP, mailbox secrets, or refresh-token values.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopGrant {
    DeviceCode,
    PkceAuthorizationCode,
}

impl DesktopGrant {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeviceCode => "device_code",
            Self::PkceAuthorizationCode => "authorization_code",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesktopSessionCommand {
    RegisterDevice {
        client_id: String,
        occurred_at: String,
    },
    RevokeDevice {
        session_id: String,
        actor_user_id: String,
        occurred_at: String,
    },
    IssueDesktopSession {
        user_id: String,
        session_id: String,
        grant: DesktopGrant,
        occurred_at: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopSessionEvent {
    DeviceRegistered {
        client_id: String,
        occurred_at: String,
    },
    DeviceRevoked {
        session_id: String,
        actor_user_id: String,
        occurred_at: String,
    },
    DesktopSessionIssued {
        user_id: String,
        session_id: String,
        grant: DesktopGrant,
        occurred_at: String,
    },
}

impl DesktopSessionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::DeviceRegistered { .. } => "desktop.device_registered",
            Self::DeviceRevoked { .. } => "desktop.device_revoked",
            Self::DesktopSessionIssued { .. } => "desktop.session_issued",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesktopSessionError {
    Invalid { reason: String },
}

impl std::fmt::Display for DesktopSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid { reason } => write!(f, "{reason}"),
        }
    }
}

impl DesktopSessionCommand {
    pub fn handle(self) -> Result<DesktopSessionEvent, DesktopSessionError> {
        match self {
            Self::RegisterDevice {
                client_id,
                occurred_at,
            } => {
                if client_id.trim().is_empty() {
                    return Err(DesktopSessionError::Invalid {
                        reason: "client_id is required".to_owned(),
                    });
                }
                Ok(DesktopSessionEvent::DeviceRegistered {
                    client_id,
                    occurred_at,
                })
            }
            Self::RevokeDevice {
                session_id,
                actor_user_id,
                occurred_at,
            } => {
                if session_id.trim().is_empty() {
                    return Err(DesktopSessionError::Invalid {
                        reason: "session_id is required".to_owned(),
                    });
                }
                if actor_user_id.trim().is_empty() {
                    return Err(DesktopSessionError::Invalid {
                        reason: "actor_user_id is required".to_owned(),
                    });
                }
                Ok(DesktopSessionEvent::DeviceRevoked {
                    session_id,
                    actor_user_id,
                    occurred_at,
                })
            }
            Self::IssueDesktopSession {
                user_id,
                session_id,
                grant,
                occurred_at,
            } => {
                if user_id.trim().is_empty() || session_id.trim().is_empty() {
                    return Err(DesktopSessionError::Invalid {
                        reason: "user_id and session_id are required".to_owned(),
                    });
                }
                Ok(DesktopSessionEvent::DesktopSessionIssued {
                    user_id,
                    session_id,
                    grant,
                    occurred_at,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_desktop_session_never_carries_tokens() {
        let event = DesktopSessionCommand::IssueDesktopSession {
            user_id: "user-1".into(),
            session_id: "sess-1".into(),
            grant: DesktopGrant::DeviceCode,
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
        .handle()
        .unwrap();
        let encoded = format!("{event:?}");
        assert!(!encoded.contains("refresh"));
        assert!(!encoded.contains("access_token"));
        assert_eq!(event.event_type(), "desktop.session_issued");
    }

    #[test]
    fn register_and_revoke_device_round_trip() {
        let registered = DesktopSessionCommand::RegisterDevice {
            client_id: "buwiz-desktop".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
        .handle()
        .unwrap();
        assert!(matches!(
            registered,
            DesktopSessionEvent::DeviceRegistered { .. }
        ));
        let revoked = DesktopSessionCommand::RevokeDevice {
            session_id: "sess-1".into(),
            actor_user_id: "user-1".into(),
            occurred_at: "2026-01-01T00:00:01Z".into(),
        }
        .handle()
        .unwrap();
        assert_eq!(revoked.event_type(), "desktop.device_revoked");
    }
}
