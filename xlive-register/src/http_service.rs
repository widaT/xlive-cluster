use crate::register::{
    IncomingMessage,
    OneshotMsg::{GetAppsMap, GetServers},
    OneshotMsgKind,
};
use anyhow::Result;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use serde_json::json;
use std::convert::Infallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
async fn monitor(
    handle: UnboundedSender<IncomingMessage>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    let path = req.uri().path();

    if path.is_empty() || (!path.eq("/servers_info") && !path.eq("/channels_info")) {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap());
    }

    let msg_kind = if path.eq("/servers_info") {
        OneshotMsgKind::GetServers
    } else {
        OneshotMsgKind::GetAppsMap
    };

    let (request, receviver) = oneshot::channel();
    handle
        .send(IncomingMessage::Oneshot(msg_kind, request))
        .map_err(|_| anyhow::anyhow!("ChannelJoinFailed"))?;

    let mut resp = json!({});
    if let Ok(data) = receviver.await {
        resp = match data {
            GetServers(servers) => {
                json!({ "servers": servers })
            }
            GetAppsMap(channels) => {
                json!({ "channels": channels })
            }
        };
    }
    let mut res = Response::new(Body::from(resp.to_string()));
    res.headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    Ok(res)
}

pub struct Service {
    handle: UnboundedSender<IncomingMessage>,
}

impl Service {
    pub fn new(handle: UnboundedSender<IncomingMessage>) -> Self {
        Self { handle }
    }
    pub async fn run(&self) {
        let handle_cp = self.handle.clone();
        let make_service = make_service_fn(move |_| {
            let handle_cp = handle_cp.clone();
            async move { Ok::<_, Infallible>(service_fn(move |req| monitor(handle_cp.clone(), req))) }
        });
        let addr = "[::]:3033".parse().unwrap();
        let server = Server::bind(&addr).serve(make_service);
        log::info!("register http service Listening on http://{}", addr);
        _ = server.await;
    }
}
