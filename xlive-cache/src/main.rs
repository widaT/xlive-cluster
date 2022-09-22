use anyhow::Result;
use chrono::Local;
use core::ManagerHandle;
use core::Upstream;
use std::io::Write;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
#[cfg(feature = "monitor")]
use xlive_cache::monitor::Service;
use xlive_cache::{conn::Connection, manager::Manager};

#[derive(Debug, StructOpt)]
#[structopt(name = "xlive-cache")]
struct Opt {
    #[structopt(short = "r", long = "register", default_value = "127.0.0.1:9336")]
    register: String,

    #[structopt(short = "o", long = "origin", default_value = "127.0.0.1:9878")]
    origin: String,
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

    let mut upstream: Option<Upstream> = None;
    if !opt.register.is_empty() {
        upstream = Some(Upstream::Register(opt.register));
    } else if !opt.origin.is_empty() {
        upstream = Some(Upstream::Addr(opt.origin));
    }

    if upstream.is_none() {
        log::error!("upstream is empty");
        std::process::exit(-1);
    }

    let manager = Manager::new(true, upstream.unwrap());
    let manager_handle = manager.handle();
    tokio::spawn(manager.run());

    #[cfg(feature = "monitor")]
    {
        let manager_handle_cp = manager_handle.clone();
        tokio::spawn(async move {
            _ = Service::new(manager_handle_cp).run().await;
        });
    }

    let listener = TcpListener::bind("0.0.0.0:9888").await?;
    log::info!(
        "xilve cache service is running,Listening for connections on {}",
        listener.local_addr()?
    );
    let mut count: u32 = 0;
    loop {
        let (stream, _) = listener.accept().await?;
        let handle = manager_handle.clone();
        tokio::spawn(async move {
            if let Err(e) = process(stream, count, handle).await {
                log::error!("failed to process connection; error = {}", e);
            }
        });
        count += 1;
    }
}
async fn process(stream: TcpStream, stream_id: u32, handle: ManagerHandle) -> Result<()> {
    Connection::new(stream, stream_id, handle).run().await?;
    Ok(())
}
