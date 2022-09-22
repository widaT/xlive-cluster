#![warn(unused_mut)]
use anyhow::Result;
use chrono::Local;
use core::Upstream;
use std::io::Write;
use structopt::StructOpt;

#[cfg(feature = "http-flv")]
use xlive_edge::http_flv;
use xlive_edge::manager::Manager;
use xlive_edge::monitor;
use xlive_edge::service::Service;

#[derive(Debug, StructOpt)]
#[structopt(name = "xlive-edge")]
struct Opt {
    #[structopt(short = "r", long = "register", default_value = "")]
    register: String,

    #[structopt(short = "o", long = "origin", default_value = "127.0.0.1:9878")]
    origin: String,

    #[structopt(short = "c", long = "cache", default_value = "")]
    cache: String,

    #[structopt(short = "b", long = "bind", default_value = "[::]:1935")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.module_path().unwrap_or("<unnamed>"),
                &record.args()
            )
        })
        .init();

    let opt = Opt::from_args();
    log::info!("{:?}", opt);

    if opt.origin.is_empty() {
        panic!("origin is empty")
    }

    let upstream;
    if !opt.register.is_empty() {
        upstream = Some(Upstream::Register(opt.register));
    } else if !opt.cache.is_empty() {
        upstream = Some(Upstream::Addr(opt.cache.clone()));
    } else {
        upstream = Some(Upstream::Addr(opt.origin.clone()));
    }

    if upstream.is_none() {
        log::error!("upstream is empty");
        std::process::exit(-1);
    }

    let mut handles = Vec::new();

    let manager = Manager::new(true, upstream.unwrap(), opt.origin);
    let manager_handle = manager.handle();
    handles.push(tokio::spawn(manager.run()));

    #[cfg(feature = "http-flv")]
    {
        let manager_handle_t = manager_handle.clone();
        handles.push(tokio::spawn(async {
            http_flv::Service::new(manager_handle_t).run().await;
        }));
    }

    #[cfg(feature = "monitor")]
    {
        let manager_handle_cp = manager_handle.clone();
        tokio::spawn(async move {
            _ = monitor::Service::new(manager_handle_cp).run().await;
        });
    }

    handles.push(tokio::spawn(Service::new(manager_handle, opt.bind).run()));

    for handle in handles {
        handle.await?;
    }
    Ok(())
}
