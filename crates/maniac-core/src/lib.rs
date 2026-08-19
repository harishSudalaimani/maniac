use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSnapshot {
    pub hostname: String,
    pub kernel: String,
    pub architecture: String,
    pub uptime_seconds: Option<u64>,
    pub cpu_count: usize,
    pub memory: MemorySnapshot,
    pub processes: Vec<ProcessSnapshot>,
    pub connections: Vec<ConnectionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub total_bytes: u64,
    pub available_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: Option<String>,
    pub executable: Option<String>,
    pub command_line: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSnapshot {
    pub protocol: String,
    pub local_address: String,
    pub remote_address: String,
    pub state: Option<String>,
    pub inode: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub created: Option<String>,
    pub host_pid: Option<u32>,
    pub process: Option<ProcessSnapshot>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub networks: Vec<ContainerNetwork>,
    pub ecs: Option<EcsContainerIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcsContainerIdentity {
    pub docker_id: Option<String>,
    pub task_arn: Option<String>,
    pub task_id: Option<String>,
    pub cluster: Option<String>,
    pub service: Option<String>,
    pub task_definition: Option<String>,
    pub container_name: Option<String>,
    pub desired_status: Option<String>,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcsTaskSnapshot {
    pub cluster: Option<String>,
    pub service: Option<String>,
    pub task_arn: Option<String>,
    pub task_id: Option<String>,
    pub task_definition: Option<String>,
    pub desired_status: Option<String>,
    pub last_status: Option<String>,
    pub containers: Vec<EcsContainerIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerNetwork {
    pub name: String,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}
