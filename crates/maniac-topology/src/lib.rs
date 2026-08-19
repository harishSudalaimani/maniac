use maniac_core::{ContainerSnapshot, HostSnapshot};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TopologyNode {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TopologyEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub protocol: Option<String>,
    pub state: Option<String>,
}

pub fn build(host: &HostSnapshot, containers: &[ContainerSnapshot]) -> TopologyGraph {
    let mut graph = TopologyGraph { nodes: Vec::new(), edges: Vec::new() };
    let host_id = "host".to_owned();
    add_node(&mut graph, host_id.clone(), "host", host.hostname.clone());
    let mut local_ip_owner = HashMap::new();
    for container in containers {
        let container_id = format!("container:{}", container.id);
        add_node(&mut graph, container_id.clone(), "container", container.name.clone());
        for network in &container.networks {
            if let Some(ip) = &network.ip_address { local_ip_owner.insert(ip.clone(), container_id.clone()); }
        }
        if let Some(process) = &container.process {
            let process_id = format!("process:{}", process.pid);
            add_node(&mut graph, process_id.clone(), "process", process.command_line.clone().or(process.name.clone()).unwrap_or_else(|| process.pid.to_string()));
            graph.edges.push(TopologyEdge { source: container_id.clone(), target: process_id, relation: "contains".into(), protocol: None, state: None });
        }
        if let Some(ecs) = &container.ecs {
            if let Some(service) = &ecs.service {
                let service_id = format!("service:{service}");
                add_node(&mut graph, service_id.clone(), "service", service.clone());
                graph.edges.push(TopologyEdge { source: service_id, target: container_id.clone(), relation: "runs".into(), protocol: None, state: None });
            }
        }
    }
    for connection in &host.connections {
        let local_ip = connection.local_address.rsplit_once(':').map(|(ip, _)| ip).unwrap_or(connection.local_address.as_str());
        let source = local_ip_owner.get(local_ip).cloned().unwrap_or_else(|| host_id.clone());
        let target = format!("endpoint:{}", connection.remote_address);
        add_node(&mut graph, target.clone(), "endpoint", connection.remote_address.clone());
        graph.edges.push(TopologyEdge { source, target, relation: "connects_to".into(), protocol: Some(connection.protocol.clone()), state: connection.state.clone() });
    }
    graph
}

fn add_node(graph: &mut TopologyGraph, id: String, kind: &str, label: String) {
    if !graph.nodes.iter().any(|node| node.id == id) { graph.nodes.push(TopologyNode { id, kind: kind.into(), label }); }
}
