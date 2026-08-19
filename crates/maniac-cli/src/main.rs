use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "maniac", about = "Live infrastructure investigation")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Local,
    Containers,
    Ecs { #[arg(long)] endpoint: Option<String> },
    Topology,
    Otel { #[arg(long)] file: String },
    Top { resource: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    maniac_linux::require_linux()?;
    let snapshot = maniac_linux::collect_host()?;
    match cli.command {
        Some(Command::Topology) => {
            let mut containers = maniac_docker::collect_containers(&snapshot.processes).unwrap_or_default();
            if let Some(endpoint) = maniac_ecs::metadata_endpoint() {
                if let Ok(task) = maniac_ecs::collect_task(Some(&endpoint)) { maniac_ecs::enrich_containers(&mut containers, &task); }
            }
            let graph = maniac_topology::build(&snapshot, &containers);
            if cli.json { println!("{}", serde_json::to_string_pretty(&graph)?); }
            else {
                println!("TOPOLOGY: {} nodes, {} edges", graph.nodes.len(), graph.edges.len());
                for edge in graph.edges { println!("{} --{}--> {}{}", label_for(&graph.nodes, &edge.source), edge.relation, label_for(&graph.nodes, &edge.target), edge.state.map(|s| format!(" [{s}]")).unwrap_or_default()); }
            }
        }
        Some(Command::Containers) | Some(Command::Top { .. }) => {
            let mut containers = maniac_docker::collect_containers(&snapshot.processes)?;
            if let Some(endpoint) = maniac_ecs::metadata_endpoint() {
                if let Ok(task) = maniac_ecs::collect_task(Some(&endpoint)) {
                    maniac_ecs::enrich_containers(&mut containers, &task);
                }
            }
            if cli.json { println!("{}", serde_json::to_string_pretty(&containers)?); }
            else {
                println!("CONTAINERS: {}", containers.len());
                println!("{:<14} {:<24} {:<18} {:>10} {:>12} {:>18} PID", "ID", "NAME", "STATUS", "CPU", "MEMORY", "NETWORK");
                for container in containers {
                    let memory = container.memory_bytes.map(format_bytes).unwrap_or_else(|| "-".into());
                    let network = format!("↓{} ↑{}", format_bytes(container.rx_bytes), format_bytes(container.tx_bytes));
                    let cpu = container.cpu_percent.map(|v| format!("{v:.1}%")).unwrap_or_else(|| "-".into());
                    let pid = container.host_pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
                    println!("{:<14} {:<24} {:<18} {:>10} {:>12} {:>18} {}", container.id, container.name, container.status, cpu, memory, network, pid);
                }
            }
        }
        Some(Command::Ecs { endpoint }) => {
            let task = maniac_ecs::collect_task(endpoint.as_deref())?;
            if cli.json { println!("{}", serde_json::to_string_pretty(&task)?); }
            else {
                println!("ECS TASK");
                println!("Cluster: {}", task.cluster.as_deref().unwrap_or("-"));
                println!("Service: {}", task.service.as_deref().unwrap_or("-"));
                println!("Task: {}", task.task_id.as_deref().or(task.task_arn.as_deref()).unwrap_or("-"));
                println!("Definition: {}", task.task_definition.as_deref().unwrap_or("-"));
                println!("Status: {} → {}", task.last_status.as_deref().unwrap_or("-"), task.desired_status.as_deref().unwrap_or("-"));
                println!("Containers: {}", task.containers.len());
                for container in task.containers { println!("  {} ({})", container.container_name.as_deref().unwrap_or("-"), container.last_status.as_deref().unwrap_or("-")); }
            }
        }
        Some(Command::Otel { file }) => {
            let spans = maniac_otel::load_file(file)?;
            let mut containers = maniac_docker::collect_containers(&snapshot.processes).unwrap_or_default();
            if let Some(endpoint) = maniac_ecs::metadata_endpoint() {
                if let Ok(task) = maniac_ecs::collect_task(Some(&endpoint)) { maniac_ecs::enrich_containers(&mut containers, &task); }
            }
            let report = maniac_otel::correlate(spans, &containers);
            if cli.json { println!("{}", serde_json::to_string_pretty(&report)?); }
            else {
                println!("OTEL SPANS: {} (unmatched: {})", report.spans.len(), report.unmatched_spans);
                println!("{:<18} {:<28} {:<20} {:<16} TASK", "TRACE", "SPAN", "SERVICE", "CONTAINER");
                for correlated in report.spans {
                    println!("{:<18} {:<28} {:<20} {:<16} {}", short_id(&correlated.span.trace_id), correlated.span.name, correlated.span.service.as_deref().unwrap_or("-"), correlated.container_name.as_deref().unwrap_or("-"), correlated.ecs_task_id.as_deref().unwrap_or("-"));
                }
            }
        }
        Some(Command::Local) | None => {
            if cli.json { println!("{}", serde_json::to_string_pretty(&snapshot)?); }
            else {
                println!("MANIAC");
                println!("Host: {}", snapshot.hostname);
                println!("Processes: {}", snapshot.processes.len());
                println!("Connections: {}", snapshot.connections.len());
                println!("Memory: {} MiB total", snapshot.memory.total_bytes / 1024 / 1024);
            }
        }
    }
    Ok(())
}

fn label_for(nodes: &[maniac_topology::TopologyNode], id: &str) -> String {
    nodes.iter().find(|node| node.id == id).map(|node| node.label.clone()).unwrap_or_else(|| id.to_owned())
}

fn short_id(id: &str) -> String { id.chars().take(16).collect() }

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 { value /= 1024.0; unit += 1; }
    if unit == 0 { format!("{} {}", bytes, UNITS[unit]) } else { format!("{value:.1} {}", UNITS[unit]) }
}
