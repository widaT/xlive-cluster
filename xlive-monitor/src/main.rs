use anyhow::Result;
use chrono::Local;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use structopt::StructOpt;
use tokio::sync::mpsc::unbounded_channel;
use xlive_monitor::monitor::Monitor;
use xlive_monitor::IncomingMessage;
use xlive_monitor::{http_service::Service, spider::Task};

#[derive(Deserialize, Debug)]
struct Host {
    name: Option<String>,
    addr: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Conf {
    hosts: Option<Vec<Host>>,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "xlive-monitor")]
struct Opt {
    #[structopt(short = "c", long = "config", default_value = "config.toml")]
    config: String,
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
    log::info!("opt:{:?}", opt);

    let file_path = opt.config;
    let conf_str = fs::read_to_string(file_path)?;
    let config: Conf = toml::from_str(&conf_str)?;
    log::info!("config:{:?}", config);
    let (sender, recivicer) = unbounded_channel::<IncomingMessage>();

    let mut handles = vec![];
    let hosts = config.hosts.unwrap();
    for host in hosts {
        let sender_cp = sender.clone();
        handles.push(tokio::spawn(async move {
            Task::new(&host.name.unwrap(), &host.addr.unwrap(), sender_cp)
                .run()
                .await?;
            Ok::<(), anyhow::Error>(())
        }));
    }

    handles.push(tokio::spawn(async move {
        Monitor::new(recivicer).run().await?;
        Ok::<(), anyhow::Error>(())
    }));

    let sender_cp = sender.clone();

    handles.push(tokio::spawn(async move {
        Service::new(sender_cp).run().await?;
        Ok::<(), anyhow::Error>(())
    }));

    for h in handles {
        if let Err(e) = h.await {
            log::error!("{:?}", e);
        }
    }
    Ok(())
}
