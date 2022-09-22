use anyhow::Result;
use chrono::Local;
use core::ManagerHandle;
use std::io::Write;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
#[cfg(feature = "monitor")]
use xlive_origin::monitor::Service;
use xlive_origin::{conn::Connection, manager::Manager};

#[derive(Debug, StructOpt)]
#[structopt(name = "xlive-origin")]
struct Opt {
    #[structopt(short = "r", long = "register", default_value = "127.0.0.1:9336")]
    register: String,
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

    let register = if opt.register.is_empty() {
        None
    } else {
        Some(opt.register)
    };
    let manager = Manager::new(true, register);
    let manager_handle = manager.handle();
    tokio::spawn(manager.run());

    #[cfg(feature = "monitor")]
    {
        let manager_handle_cp = manager_handle.clone();
        tokio::spawn(async move {
            _ = Service::new(manager_handle_cp).run().await;
        });
    }

    let listener = TcpListener::bind("0.0.0.0:9878").await?;
    log::info!(
        "xilve origin service is running,Listening for connections on {}",
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
