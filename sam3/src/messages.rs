use serde::{Deserialize, Serialize};

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MsgType {
    Ready,
    Result,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReadyMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub dest: String,
}

impl ReadyMsg {
    pub fn new(dest: String) -> Self {
        Self {
            msg_type: "ready".to_string(),
            dest,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResultMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub setup_ms: f64,
    pub transfer_ms: f64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub first_byte_ms: f64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub goodput_mbps: f64,
    pub payload_bytes: i64,
    pub sha256_ok: bool,
    pub success: bool,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub error: String,
}

impl ResultMsg {
    pub fn new(role: &str) -> Self {
        Self {
            msg_type: "result".to_string(),
            role: role.to_string(),
            setup_ms: 0.0,
            transfer_ms: 0.0,
            first_byte_ms: 0.0,
            goodput_mbps: 0.0,
            payload_bytes: 0,
            sha256_ok: false,
            success: false,
            error: String::new(),
        }
    }
}
