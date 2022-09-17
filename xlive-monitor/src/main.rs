use anyhow::Result;
use chrono::Local;
use std::io::Write;
use xlive_monitor::monitor::Service;

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
    Service::new().run().await;
    Ok(())
}
