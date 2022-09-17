mod conn;
mod packet;
mod rtmp;
pub mod service;

mod channel;
pub mod manager;

#[cfg(feature = "http-flv")]
pub mod http_flv;

#[cfg(feature = "ts")]
mod transport_stream;

#[cfg(feature = "monitor")]
pub mod monitor;

const FLV_HEADER: [u8; 13] = [
    0x46, 0x4c, 0x56, 0x01, 0x05, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x00,
];
fn put_i24_be(b: &mut [u8], v: i32) {
    b[0] = (v >> 16) as u8;
    b[1] = (v >> 8) as u8;
    b[2] = v as u8;
}

fn put_i32_be(b: &mut [u8], v: i32) {
    b[0] = (v >> 24) as u8;
    b[1] = (v >> 16) as u8;
    b[2] = (v >> 8) as u8;
    b[3] = v as u8;
}
