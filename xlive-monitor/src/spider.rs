use crate::IncomingMessage;
use anyhow::{Ok, Result};
use std::{collections::HashMap, time::Duration};
use tokio::{sync::mpsc::UnboundedSender, time};

pub struct Task {
    outgoing: UnboundedSender<IncomingMessage>,
    name: String,
    url: String,
}

impl Task {
    pub fn new(name: &str, url: &str, outgoing: UnboundedSender<IncomingMessage>) -> Self {
        Self {
            outgoing,
            name: name.to_owned(),
            url: url.to_owned(),
        }
    }

    pub async fn monitor(url: &str) -> Result<HashMap<String, usize>> {
        let resp = reqwest::get(url)
            .await?
            .json::<HashMap<String, usize>>()
            .await?;

        Ok(resp)
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let mut interval = time::interval(Duration::from_secs(1));
            interval.tick().await;
            let resp = Self::monitor(&self.url).await?;
            _ = self
                .outgoing
                .send(IncomingMessage::TaskMsg((self.name.clone(), resp)));
        }
    }
}
