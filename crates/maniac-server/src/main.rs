use anyhow::Result;
use clap::Parser;
use maniac_protocol::{AgentMessage, ServerMessage, PROTOCOL_VERSION};
use std::{collections::HashMap, io::{BufReader, BufWriter}, net::{TcpListener, TcpStream}, sync::{Arc, Mutex}};

#[derive(Parser)]
#[command(name = "maniac-server", about = "Maniac in-memory multi-host server")]
struct Args {
    #[arg(long, env = "MANIAC_LISTEN", default_value = "0.0.0.0:9100")]
    listen: String,
    #[arg(long, env = "MANIAC_TOKEN")]
    token: String,
}

type State = Arc<Mutex<HashMap<String, maniac_protocol::AgentSnapshot>>>;

fn main() -> Result<()> {
    let args = Args::parse();
    let listener = TcpListener::bind(&args.listen)?;
    let state: State = Arc::new(Mutex::new(HashMap::new()));
    eprintln!("Maniac server listening on {}", args.listen);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => { let token = args.token.clone(); let state = Arc::clone(&state); std::thread::spawn(move || handle(stream, token, state)); }
            Err(error) => eprintln!("server accept error: {error}"),
        }
    }
    Ok(())
}

fn handle(stream: TcpStream, expected_token: String, state: State) {
    let writer_stream = match stream.try_clone() { Ok(stream) => stream, Err(error) => { eprintln!("server clone error: {error}"); return; } };
    let mut reader = BufReader::new(stream);
    let mut writer = BufWriter::new(writer_stream);
    let hello = match maniac_protocol::receive::<AgentMessage, _>(&mut reader) { Ok(message) => message, Err(error) => { eprintln!("server protocol error: {error}"); return; } };
    let hostname = match hello {
        AgentMessage::Hello { version, token, hostname } if version == PROTOCOL_VERSION && token == expected_token => { let _ = maniac_protocol::send(&mut writer, &ServerMessage::Accepted { version }); hostname }
        AgentMessage::Hello { .. } => { let _ = maniac_protocol::send(&mut writer, &ServerMessage::Rejected { reason: "authentication or protocol version failed".into() }); return; }
        _ => { let _ = maniac_protocol::send(&mut writer, &ServerMessage::Rejected { reason: "hello required".into() }); return; }
    };
    loop {
        match maniac_protocol::receive::<AgentMessage, _>(&mut reader) {
            Ok(AgentMessage::Snapshot { snapshot }) => {
                let host_count = snapshot.host.hostname.clone();
                if let Ok(mut state) = state.lock() { state.insert(host_count, snapshot); }
                let _ = maniac_protocol::send(&mut writer, &ServerMessage::SnapshotAccepted);
            }
            Ok(AgentMessage::Heartbeat) => {}
            Ok(AgentMessage::Hello { .. }) => {}
            Err(_) => { eprintln!("agent disconnected: {hostname}"); return; }
        }
    }
}
