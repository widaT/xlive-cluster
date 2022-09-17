use anyhow::{bail, Result};
use bytes::Bytes;
use core::{ChannelMessage, ManagerHandle};
use hyper::body::Sender;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use tokio::sync::oneshot;
async fn monitor(manager_handle: ManagerHandle, req: Request<Body>) -> Result<Response<Body>> {
    let path = req.uri().path();

    if path.is_empty() || !path.eq("/monitor") {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap());
    }
    let mut conn = Conn::new(manager_handle);
    let (sender, body) = Body::channel();
    match conn.init(sender).await {
        Ok(_) => {}
        Err(e) => {
            log::error!("{}", e);
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap());
        }
    }
    let mut res = Response::new(body);
    res.headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    Ok(res)
}

pub struct Service {
    manager_handle: ManagerHandle,
}

impl Service {
    pub fn new(manager_handle: ManagerHandle) -> Self {
        Self { manager_handle }
    }

    pub async fn run(&self) {
        let manager_handle = self.manager_handle.clone();
        let make_service = make_service_fn(move |_| {
            let manager_handle = manager_handle.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| monitor(manager_handle.clone(), req)))
            }
        });
        let addr = "[::]:3032".parse().unwrap();
        let server = Server::bind(&addr).serve(make_service);
        log::info!("monitor Listening on http://{}", addr);
        _ = server.await;
    }
}

pub struct Conn {
    manager_handle: ManagerHandle,
}

impl Conn {
    pub fn new(manager_handle: ManagerHandle) -> Self {
        Self { manager_handle }
    }
    async fn init(&mut self, mut body_sender: Sender) -> Result<()> {
        let (request, response) = oneshot::channel();
        self.manager_handle
            .send(ChannelMessage::Snapshot(request))
            .map_err(|_| anyhow::anyhow!("ChannelJoinFailed"))?;

        if let Ok(info) = response.await {
            let resp = serde_json::to_value(info)?;
            match body_sender.send_data(Bytes::from(resp.to_string())).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("send_data err {}", e);
                    bail!("send_data err {}", e);
                }
            };
        };
        Ok(())
    }
}
