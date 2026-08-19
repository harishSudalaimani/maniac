//! Optional OpenTelemetry correlation from an OTLP JSON export.
//!
//! This intentionally does not make an OTel collector a Maniac dependency.
//! The importer can consume an OTLP JSON export now; a live OTLP receiver can
//! be added later without changing the correlation model.

use anyhow::{Context, Result};
use maniac_core::ContainerSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub service: Option<String>,
    pub start_time_unix_nano: Option<u64>,
    pub end_time_unix_nano: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedSpan {
    pub span: SpanRecord,
    pub container_id: Option<String>,
    pub container_name: Option<String>,
    pub process_pid: Option<u32>,
    pub ecs_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationReport {
    pub spans: Vec<CorrelatedSpan>,
    pub unmatched_spans: usize,
}

pub fn load_file(path: impl AsRef<Path>) -> Result<Vec<SpanRecord>> {
    let content = fs::read_to_string(path)?;
    let document: Value = serde_json::from_str(&content).context("decoding OTLP JSON export")?;
    Ok(parse_document(&document))
}

pub fn correlate(spans: Vec<SpanRecord>, containers: &[ContainerSnapshot]) -> CorrelationReport {
    let mut by_service = HashMap::new();
    for container in containers {
        if let Some(service) = container.ecs.as_ref().and_then(|ecs| ecs.service.as_ref()) {
            by_service.entry(service.to_lowercase()).or_insert(container);
        }
    }
    let mut unmatched = 0;
    let correlated = spans.into_iter().map(|span| {
        let container = span.service.as_ref().and_then(|service| by_service.get(&service.to_lowercase()).copied());
        if container.is_none() { unmatched += 1; }
        CorrelatedSpan {
            span,
            container_id: container.map(|c| c.id.clone()),
            container_name: container.map(|c| c.name.clone()),
            process_pid: container.and_then(|c| c.host_pid),
            ecs_task_id: container.and_then(|c| c.ecs.as_ref().and_then(|ecs| ecs.task_id.clone())),
        }
    }).collect();
    CorrelationReport { spans: correlated, unmatched_spans: unmatched }
}

fn parse_document(document: &Value) -> Vec<SpanRecord> {
    let mut spans = Vec::new();
    for resource in document.get("resourceSpans").and_then(Value::as_array).into_iter().flatten() {
        let resource_service = attribute(resource.pointer("/resource/attributes"), "service.name");
        for scope in resource.get("scopeSpans").and_then(Value::as_array).into_iter().flatten() {
            for span in scope.get("spans").and_then(Value::as_array).into_iter().flatten() {
                let service = attribute(span.get("attributes"), "service.name").or_else(|| resource_service.clone());
                spans.push(SpanRecord {
                    trace_id: string(span, "traceId").unwrap_or_default(),
                    span_id: string(span, "spanId").unwrap_or_default(),
                    parent_span_id: string(span, "parentSpanId"),
                    name: string(span, "name").unwrap_or_else(|| "unnamed".into()),
                    service,
                    start_time_unix_nano: number(span, "startTimeUnixNano"),
                    end_time_unix_nano: number(span, "endTimeUnixNano"),
                    status: span.pointer("/status/code").and_then(Value::as_str).map(str::to_owned),
                });
            }
        }
    }
    spans
}

fn attribute(attributes: Option<&Value>, wanted: &str) -> Option<String> {
    attributes?.as_array()?.iter().find(|attribute| string(attribute, "key").as_deref() == Some(wanted)).and_then(|attribute| {
        let value = attribute.get("value")?;
        ["stringValue", "intValue", "boolValue", "doubleValue"].iter().find_map(|key| value.get(*key).map(|v| v.to_string().trim_matches('"').to_owned()))
    })
}

fn string(value: &Value, key: &str) -> Option<String> { value.get(key).and_then(Value::as_str).map(str::to_owned) }

fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
}
