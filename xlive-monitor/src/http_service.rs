use crate::IncomingMessage;
use anyhow::Result;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
async fn monitor(
    handle: UnboundedSender<IncomingMessage>,
    req: Request<Body>,
) -> Result<Response<Body>> {
    let path = req.uri().path();

    if path.is_empty() || !path.eq("/info") {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap());
    }

    let (re, rv) = oneshot::channel();

    _ = handle.send(IncomingMessage::Oneshot(re));

    let resp = rv.await?;

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

    pub async fn run(&self) -> Result<()> {
        let handle_cp = self.handle.clone();
        let make_service = make_service_fn(move |_| {
            let handle_cp = handle_cp.clone();
            async move { Ok::<_, Infallible>(service_fn(move |req| monitor(handle_cp.clone(), req))) }
        });
        let addr = "[::]:3033".parse().unwrap();
        let server = Server::bind(&addr).serve(make_service);
        log::info!("monitor http service Listening on http://{}", addr);
        server.await?;
        Ok(())
    }
}
