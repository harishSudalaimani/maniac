//! Read-only Docker Engine discovery over the local Unix socket.

use anyhow::{anyhow, Context, Result};
use maniac_core::{ContainerNetwork, ContainerSnapshot, ProcessSnapshot};
use serde_json::Value;
use std::{env, fs, io::{Read, Write}, os::unix::net::UnixStream, time::Duration};

const DEFAULT_SOCKET: &str = "/var/run/docker.sock";

pub fn collect_containers(processes: &[ProcessSnapshot]) -> Result<Vec<ContainerSnapshot>> {
    let socket = env::var("DOCKER_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET.to_owned());
    let list = request_json(&socket, "/containers/json?all=1")?;
    let items = list.as_array().ok_or_else(|| anyhow!("Docker returned an invalid container list"))?;
    items.iter().map(|summary| {
        let full_id = string(summary, "Id").ok_or_else(|| anyhow!("Docker container has no id"))?;
        let inspect = request_json(&socket, &format!("/containers/{full_id}/json"))?;
        let state = inspect.get("State").unwrap_or(&Value::Null);
        let host_pid = state.get("Pid").and_then(Value::as_u64).map(|pid| pid as u32).filter(|pid| *pid > 0);
        let process = host_pid.and_then(|pid| processes.iter().find(|p| p.pid == pid).cloned());
        let mut networks = collect_networks(&inspect);
        let stats = request_json(&socket, &format!("/containers/{full_id}/stats?stream=false")).unwrap_or(Value::Null);
        let (cpu_percent, memory_bytes, memory_limit_bytes) = collect_stats(&stats);
        if let Some(network_stats) = stats.get("networks").and_then(Value::as_object) {
            for network in &mut networks {
                if let Some(stats) = network_stats.get(&network.name) {
                    network.rx_bytes = stats.get("rx_bytes").and_then(Value::as_u64).unwrap_or(0);
                    network.tx_bytes = stats.get("tx_bytes").and_then(Value::as_u64).unwrap_or(0);
                }
            }
        }
        Ok(ContainerSnapshot {
            id: full_id.chars().take(12).collect(),
            name: string(&inspect, "Name").unwrap_or_default().trim_start_matches('/').to_owned(),
            image: inspect.pointer("/Config/Image").and_then(Value::as_str).map(str::to_owned).or_else(|| string(summary, "Image")).unwrap_or_default(),
            status: string(state, "Status").or_else(|| string(summary, "Status")).unwrap_or_default(),
            created: string(&inspect, "Created").or_else(|| string(summary, "Created")),
            host_pid, process, cpu_percent, memory_bytes, memory_limit_bytes,
            rx_bytes: networks.iter().map(|n| n.rx_bytes).sum(),
            tx_bytes: networks.iter().map(|n| n.tx_bytes).sum(),
            networks,
            ecs: None,
        })
    }).collect()
}

fn collect_networks(inspect: &Value) -> Vec<ContainerNetwork> {
    inspect.pointer("/NetworkSettings/Networks").and_then(Value::as_object).map(|networks| networks.iter().map(|(name, network)| ContainerNetwork {
        name: name.clone(), ip_address: string(network, "IPAddress"), gateway: string(network, "Gateway"), rx_bytes: 0, tx_bytes: 0,
    }).collect()).unwrap_or_default()
}

fn collect_stats(stats: &Value) -> (Option<f64>, Option<u64>, Option<u64>) {
    let memory = stats.get("memory_stats").unwrap_or(&Value::Null);
    let usage = memory.get("usage").and_then(Value::as_u64);
    let limit = memory.get("limit").and_then(Value::as_u64);
    let cpu = stats.pointer("/cpu_stats/cpu_usage/total_usage").and_then(Value::as_f64);
    let precpu = stats.pointer("/precpu_stats/cpu_usage/total_usage").and_then(Value::as_f64);
    let system = stats.pointer("/cpu_stats/system_cpu_usage").and_then(Value::as_f64);
    let presystem = stats.pointer("/precpu_stats/system_cpu_usage").and_then(Value::as_f64);
    let online = stats.pointer("/cpu_stats/online_cpus").and_then(Value::as_f64).unwrap_or(1.0);
    let percent = match (cpu, precpu, system, presystem) {
        (Some(c), Some(pc), Some(s), Some(ps)) if s > ps && c >= pc => Some((c - pc) / (s - ps) * online * 100.0),
        _ => None,
    };
    (percent, usage, limit)
}

fn string(value: &Value, key: &str) -> Option<String> { value.get(key).and_then(Value::as_str).map(str::to_owned) }

fn request_json(socket: &str, path: &str) -> Result<Value> {
    if !fs::metadata(socket).is_ok() { return Err(anyhow!("Docker socket not found at {socket}")); }
    let mut stream = UnixStream::connect(socket).with_context(|| format!("connecting to Docker socket {socket}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    write!(stream, "GET {path} HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n\r\n")?;
    stream.flush()?;
    let mut response = Vec::new(); stream.read_to_end(&mut response)?;
    let response = String::from_utf8_lossy(&response);
    let (headers, body) = response.split_once("\r\n\r\n").ok_or_else(|| anyhow!("invalid Docker response"))?;
    let status = headers.lines().next().unwrap_or_default();
    if !status.contains(" 200 ") { return Err(anyhow!("Docker request failed: {status}")); }
    serde_json::from_str(body).context("decoding Docker response")
}
