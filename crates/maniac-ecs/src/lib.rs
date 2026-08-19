//! ECS task metadata integration.
//!
//! The metadata endpoint is local to an ECS task and requires no AWS
//! credentials. AWS SDK-backed cluster-wide discovery remains a later server
//! concern; local inspection continues to work when metadata is unavailable.

use anyhow::{anyhow, Context, Result};
use maniac_core::{ContainerSnapshot, EcsContainerIdentity, EcsTaskSnapshot};
use serde_json::Value;
use std::{env, io::{Read, Write}, net::TcpStream, time::Duration};

pub fn metadata_endpoint() -> Option<String> {
    env::var("ECS_METADATA_URL").ok()
        .or_else(|| env::var("ECS_CONTAINER_METADATA_URI_V4").ok().map(|url| format!("{url}/task")))
        .or_else(|| env::var("ECS_CONTAINER_METADATA_URI").ok().map(|url| format!("{url}/task")))
}

pub fn collect_task(endpoint: Option<&str>) -> Result<EcsTaskSnapshot> {
    let endpoint = endpoint.map(str::to_owned).or_else(metadata_endpoint)
        .ok_or_else(|| anyhow!("ECS metadata endpoint is not configured"))?;
    let document = get_json(&endpoint)?;
    let task_arn = string(&document, "TaskARN");
    let task_id = task_arn.as_deref().and_then(|arn| arn.rsplit('/').next()).map(str::to_owned);
    let cluster = string(&document, "Cluster");
    let task_definition = match (string(&document, "Family"), string(&document, "Revision")) {
        (Some(family), Some(revision)) => Some(format!("{family}:{revision}")),
        _ => None,
    };
    let task_service = string(&document, "ServiceName");
    let empty_containers = Vec::new();
    let containers_value = document.get("Containers").and_then(Value::as_array).unwrap_or(&empty_containers);
    let containers = containers_value.iter().map(|container| {
        let labels = container.get("Labels").unwrap_or(&Value::Null);
        EcsContainerIdentity {
            docker_id: string(container, "DockerId"), task_arn: task_arn.clone(), task_id: task_id.clone(), cluster: cluster.clone(),
            service: string(labels, "com.amazonaws.ecs.service-name").or_else(|| task_service.clone()),
            task_definition: task_definition.clone(), container_name: string(container, "Name"),
            desired_status: string(container, "DesiredStatus"), last_status: string(container, "KnownStatus"),
        }
    }).collect::<Vec<_>>();
    Ok(EcsTaskSnapshot {
        cluster, service: task_service.or_else(|| containers.iter().find_map(|c| c.service.clone())),
        task_arn, task_id, task_definition,
        desired_status: string(&document, "DesiredStatus"), last_status: string(&document, "KnownStatus"),
        containers,
    })
}

pub fn enrich_containers(containers: &mut [ContainerSnapshot], task: &EcsTaskSnapshot) {
    for container in containers {
        let identity = task.containers.iter().find(|ecs| {
            ecs.container_name.as_deref() == Some(container.name.as_str())
                || ecs.docker_id.as_deref().map(|id| id.starts_with(&container.id)).unwrap_or(false)
        }).cloned();
        if identity.is_some() { container.ecs = identity; }
    }
}

fn string(value: &Value, key: &str) -> Option<String> { value.get(key).and_then(Value::as_str).map(str::to_owned) }

fn get_json(url: &str) -> Result<Value> {
    let without_scheme = url.strip_prefix("http://").ok_or_else(|| anyhow!("ECS metadata endpoint must use http://"))?;
    let (authority, path) = without_scheme.split_once('/').map_or((without_scheme, "/".to_owned()), |(host, path)| (host, format!("/{path}")));
    let (host, port) = authority.rsplit_once(':').map_or((authority, 80), |(host, port)| (host, port.parse().unwrap_or(80)));
    let mut stream = TcpStream::connect((host, port)).with_context(|| format!("connecting to ECS metadata endpoint {url}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n")?;
    stream.flush()?;
    let mut response = Vec::new(); stream.read_to_end(&mut response)?;
    let response = String::from_utf8_lossy(&response);
    let (headers, body) = response.split_once("\r\n\r\n").ok_or_else(|| anyhow!("invalid ECS metadata response"))?;
    let status = headers.lines().next().unwrap_or_default();
    if !status.contains(" 200 ") { return Err(anyhow!("ECS metadata request failed: {status}")); }
    serde_json::from_str(body).context("decoding ECS metadata")
}
