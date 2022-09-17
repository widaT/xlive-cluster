use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize)]
pub enum RegisterKind {
    Set = 1u8,
    Get = 2u8,
    Delete = 3u8,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Register {
    pub kind: RegisterKind,
    pub channel_name: String,
}

impl TryFrom<&[u8]> for Register {
    type Error = anyhow::Error;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let value: Register = bincode::deserialize(bytes)?;
        Ok(value)
    }
}

impl TryFrom<Register> for Bytes {
    type Error = anyhow::Error;
    fn try_from(v: Register) -> Result<Self, Self::Error> {
        let buf: Vec<u8> = bincode::serialize(&v)?;
        Ok(Bytes::from(buf))
    }
}

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize)]
pub enum RegisterRespKind {
    OK = 1,
    NOFOUND = 2,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterResp {
    pub kind: RegisterRespKind,
    pub payload: String,
}

impl TryFrom<&[u8]> for RegisterResp {
    type Error = anyhow::Error;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let value: RegisterResp = bincode::deserialize(bytes)?;
        Ok(value)
    }
}

impl TryFrom<RegisterResp> for Bytes {
    type Error = anyhow::Error;
    fn try_from(v: RegisterResp) -> Result<Self, Self::Error> {
        let buf: Vec<u8> = bincode::serialize(&v)?;
        Ok(Bytes::from(buf))
    }
}
