use serde::{Deserialize, Serialize};

/// A profile an agent connection reaches.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProfileRef {
    pub profile_id: String,
    pub profile_name: String,
    pub tin_last4: String,
}

/// An AI agent the caller allowed on some of their profiles.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConnectionView {
    pub connection_id: String,
    pub client_id: String,
    /// The name the app registered with. Apps name themselves, so this is
    /// not proof of who made the app.
    pub client_name: String,
    pub permissions: Vec<String>,
    /// `read_only`, `safe` or `full` when the permissions match one exactly.
    #[serde(default)]
    pub preset: Option<String>,
    pub profiles: Vec<AgentProfileRef>,
    /// The owner allowed this agent to leave filings for the owner's computer.
    /// The server still never contacts BIR.
    #[serde(default)]
    pub may_queue: bool,
    pub created_at: String,
    #[serde(default)]
    pub last_used_at: Option<String>,
}

/// What the owner's app reports after it picks up a device filing.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceFilingReportRequest {
    pub status: String,
    #[serde(default)]
    pub error_summary: String,
    #[serde(default)]
    pub field_errors: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConnectionListResponse {
    pub connections: Vec<AgentConnectionView>,
}
