use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedReceiver;
use crate::IncomingMessage;
use anyhow::Result;

pub struct Monitor {
    state: HashMap<String, HashMap<String, usize>>,
    incoming: UnboundedReceiver<IncomingMessage>,
}

impl Monitor {
    pub fn new(incoming: UnboundedReceiver<IncomingMessage>) -> Self {
        Self {
            state: HashMap::new(),
            incoming,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(msg) = self.incoming.recv().await {
            match msg {
                IncomingMessage::TaskMsg((name, value)) => {
                    self.state.insert(name, value);
                }
                IncomingMessage::Oneshot(sender) => {
                    let value = serde_json::json!(self.state);
                    _ = sender.send(value);
                }
            }
        }
        Ok(())
    }
}
