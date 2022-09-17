use anyhow::{bail, Result};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
async fn monitor(req: Request<Body>) -> Result<Response<Body>> {
    let path = req.uri().path();

    if path.is_empty() || !path.eq("/monitor") {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap());
    }

    //comming soon

    let mut res = Response::new(Body::from(""));
    res.headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    Ok(res)
}

pub struct Service {}

impl Service {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&self) {
        let make_service = make_service_fn(move |_| async move {
            Ok::<_, Infallible>(service_fn(move |req| monitor(req)))
        });
        let addr = "[::]:3033".parse().unwrap();
        let server = Server::bind(&addr).serve(make_service);
        log::info!("monitor http service Listening on http://{}", addr);
        _ = server.await;
    }
}
