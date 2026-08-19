use anyhow::Result;
use maniac_core::{ConnectionSnapshot, HostSnapshot, MemorySnapshot, ProcessSnapshot};
use std::{fs, path::Path};

pub fn collect_host() -> Result<HostSnapshot> {
    let hostname = fs::read_to_string("/etc/hostname")?.trim().to_owned();
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")?.trim().to_owned();
    let architecture = std::env::consts::ARCH.to_owned();
    let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let processes = collect_processes();
    let connections = collect_proc_net("/proc/net/tcp", "tcp");
    Ok(HostSnapshot {
        hostname,
        kernel,
        architecture,
        uptime_seconds: read_uptime(),
        cpu_count,
        memory: collect_memory(),
        processes,
        connections,
    })
}

fn collect_processes() -> Vec<ProcessSnapshot> {
    let Ok(entries) = fs::read_dir("/proc") else { return Vec::new() };
    entries.filter_map(|entry| {
        let path = entry.ok()?.path();
        let pid = path.file_name()?.to_str()?.parse().ok()?;
        let status = fs::read_to_string(path.join("status")).ok()?;
        let field = |name: &str| status.lines().find_map(|line| line.strip_prefix(name).map(str::trim).map(str::to_owned));
        Some(ProcessSnapshot {
            pid,
            ppid: field("PPid:").and_then(|v| v.parse().ok()),
            name: field("Name:"),
            executable: fs::read_link(path.join("exe")).ok().map(|p| p.display().to_string()),
            command_line: fs::read(path.join("cmdline")).ok().map(|v| String::from_utf8_lossy(&v).replace('\0', " ").trim().to_owned()),
            user: None,
        })
    }).collect()
}

fn collect_memory() -> MemorySnapshot {
    let text = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let value = |key: &str| text.lines().find_map(|line| line.strip_prefix(key)?.split_whitespace().next()?.parse::<u64>().ok()).map(|kb| kb * 1024).unwrap_or(0);
    MemorySnapshot { total_bytes: value("MemTotal:"), available_bytes: Some(value("MemAvailable:")) }
}

fn read_uptime() -> Option<u64> {
    fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse::<f64>().ok().map(|v| v as u64)
}

fn collect_proc_net(path: impl AsRef<Path>, protocol: &str) -> Vec<ConnectionSnapshot> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    text.lines().skip(1).filter_map(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 4 { return None }
        Some(ConnectionSnapshot { protocol: protocol.into(), local_address: fields[1].into(), remote_address: fields[2].into(), state: Some(fields[3].into()), inode: fields.get(9).and_then(|v| v.parse().ok()) })
    }).collect()
}

pub fn require_linux() -> Result<()> {
    if !cfg!(target_os = "linux") { anyhow::bail!("local collection currently requires Linux") }
    Ok(())
}
