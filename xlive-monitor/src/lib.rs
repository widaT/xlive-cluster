use std::collections::HashMap;

use tokio::sync::oneshot;

pub mod http_service;
pub mod monitor;
pub mod spider;

pub enum IncomingMessage {
    TaskMsg((String, HashMap<String, usize>)),
    Oneshot(oneshot::Sender<serde_json::Value>),
}
