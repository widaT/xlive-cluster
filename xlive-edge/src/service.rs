use crate::conn::Connection;
use anyhow::Result;
use core::ManagerHandle;
use tokio::net::{TcpListener, TcpStream};

pub struct Service {
    manager_handle: ManagerHandle,
    client_id: u64,
    addr: String,
}

impl Service {
    pub fn new(manager_handle: ManagerHandle, addr: String) -> Self {
        Self {
            manager_handle,
            client_id: 0,
            addr,
        }
    }
    pub async fn run(mut self) {
        if let Err(err) = self.handle_rtmp().await {
            log::error!("{}", err);
        }
    }

    async fn handle_rtmp(&mut self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        log::info!("Listening for RTMP connections on {}", &self.addr);
        loop {
            let (tcp_stream, _addr) = listener.accept().await?;
            self.process(tcp_stream);
            self.client_id += 1;
        }
    }

    fn process(&self, stream: TcpStream) {
        log::info!("New client connection: {}", &self.client_id);
        let id = self.client_id;
        let conn = Connection::new(id, stream, self.manager_handle.clone());

        tokio::spawn(async move {
            if let Err(err) = conn.run().await {
                log::error!("{}", err);
            }
        });
    }
}
