use anyhow::{Context, Result};
use maniac_core::{ContainerSnapshot, HostSnapshot};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMessage {
    Hello { version: u16, token: String, hostname: String },
    Snapshot { snapshot: AgentSnapshot },
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub host: HostSnapshot,
    pub containers: Vec<ContainerSnapshot>,
    pub collected_at_unix: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Accepted { version: u16 },
    Rejected { reason: String },
    SnapshotAccepted,
}

pub fn send<T: Serialize, W: Write>(writer: &mut W, message: &T) -> Result<()> {
    serde_json::to_writer(&mut *writer, message)?;
    writer.write_all(b"\n")?;
    writer.flush().context("flushing Maniac protocol message")
}

pub fn receive<T: for<'de> Deserialize<'de>, R: BufRead>(reader: &mut R) -> Result<T> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 { anyhow::bail!("peer closed the connection") }
    serde_json::from_str(line.trim()).context("decoding Maniac protocol message")
}
