use anyhow::{Context, Result};
use clap::Parser;
use maniac_protocol::{AgentMessage, AgentSnapshot, ServerMessage, PROTOCOL_VERSION};
use std::{io::{BufReader, BufWriter}, net::TcpStream, thread, time::{Duration, SystemTime, UNIX_EPOCH}};

#[derive(Parser)]
#[command(name = "maniac-agent", about = "Maniac outbound infrastructure collector")]
struct Args {
    #[arg(long, env = "MANIAC_SERVER", default_value = "127.0.0.1:9100")]
    server: String,
    #[arg(long, env = "MANIAC_TOKEN")]
    token: String,
    #[arg(long, env = "MANIAC_INTERVAL", default_value_t = 10)]
    interval_seconds: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    maniac_linux::require_linux()?;
    loop {
        if let Err(error) = publish(&args) { eprintln!("agent: {error:#}"); }
        thread::sleep(Duration::from_secs(args.interval_seconds.max(1)));
    }
}

fn publish(args: &Args) -> Result<()> {
    let snapshot = maniac_linux::collect_host()?;
    let containers = maniac_docker::collect_containers(&snapshot.processes).unwrap_or_default();
    let stream = TcpStream::connect(&args.server).with_context(|| format!("connecting to Maniac server {}", args.server))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut writer = BufWriter::new(stream.try_clone()?);
    let mut reader = BufReader::new(stream);
    maniac_protocol::send(&mut writer, &AgentMessage::Hello { version: PROTOCOL_VERSION, token: args.token.clone(), hostname: snapshot.hostname.clone() })?;
    match maniac_protocol::receive::<ServerMessage, _>(&mut reader)? {
        ServerMessage::Accepted { .. } => {}
        ServerMessage::Rejected { reason } => anyhow::bail!("server rejected agent: {reason}"),
        ServerMessage::SnapshotAccepted => anyhow::bail!("unexpected server response"),
    }
    let message = AgentMessage::Snapshot { snapshot: AgentSnapshot { host: snapshot, containers, collected_at_unix: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() } };
    maniac_protocol::send(&mut writer, &message)?;
    match maniac_protocol::receive::<ServerMessage, _>(&mut reader)? {
        ServerMessage::SnapshotAccepted => Ok(()),
        ServerMessage::Rejected { reason } => anyhow::bail!("snapshot rejected: {reason}"),
        ServerMessage::Accepted { .. } => anyhow::bail!("unexpected server response"),
    }
}
