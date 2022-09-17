pub mod message;
pub mod register;
pub mod transport;

pub type Event = &'static str;
pub type AppName = String;
pub type StreamKey = String;

pub use self::transport::{
    trigger_channel, ChannelMessage, Handle, ManagerHandle, Message, Watcher,
};

//define the kind of upstream for cache and edge
#[derive(Debug)]
pub enum Upstream {
    Register(String),
    Addr(String),
}
