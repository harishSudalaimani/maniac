# Maniac

Maniac is a lightweight, read-only infrastructure investigation tool for answering:

> What the hell is happening on my infrastructure right now?

It correlates hosts, containers, processes, sockets, and network connections into one investigation workflow. The initial release targets Linux and works locally without AWS credentials or application instrumentation.

## Current status

This repository now contains the local-to-multi-host foundation through Phase 4:

- process discovery from `/proc`
- host CPU and memory information
- TCP socket collection from `/proc/net`
- Docker container discovery through the Engine Unix socket
- container host-PID ↔ process correlation
- container memory/CPU/network statistics
- JSON output for automation
- ECS task metadata and Docker-container correlation
- outbound agent with persistent authenticated connections
- in-memory server tracking the latest snapshot from each host
- topology graph connecting hosts, containers, processes, services, and remote endpoints
- optional OTLP JSON trace import and service/container correlation
- terminal-friendly `local`, `containers`, `ecs`, and `top containers` commands

The multi-host server is intentionally small at this stage: it accepts newline-delimited typed JSON snapshots and keeps current state in memory. A query API, TLS certificates, streaming subscriptions, TUI, and topology correlation remain follow-up work.

## Quick start

```bash
cargo run -p maniac-cli -- local --json
cargo run -p maniac-cli -- containers
cargo run -p maniac-cli -- --json containers
cargo run -p maniac-cli -- ecs
cargo run -p maniac-cli -- --json ecs
```

Docker collection uses `/var/run/docker.sock` by default. Set `DOCKER_SOCKET` to use another socket. If Docker is not installed or accessible, `maniac local` continues to work.

On ECS, `maniac ecs` discovers the task metadata endpoint from `ECS_CONTAINER_METADATA_URI_V4` or `ECS_CONTAINER_METADATA_URI`. For local testing, pass `--endpoint http://127.0.0.1:51679/v4/task` or set `ECS_METADATA_URL`. AWS credentials are not required for this local task-level integration.

## Multi-host mode

Start the server with a shared token:

```bash
MANIAC_TOKEN=change-me cargo run -p maniac-server
```

Run an agent on each Linux host. Agents make outbound connections to the server and reconnect on the next collection interval if the server is unavailable:

```bash
MANIAC_SERVER=server.example.com:9100 \
MANIAC_TOKEN=change-me \
cargo run -p maniac-agent
```

The protocol is versioned and the server rejects invalid tokens or incompatible versions. It is not TLS-enabled yet; keep this MVP on a private network until the TLS transport layer is added.

## Topology

Build a local graph from the current host connections and discovered containers:

```bash
cargo run -p maniac-cli -- topology
cargo run -p maniac-cli -- --json topology > topology.json
```

The graph includes host, container, process, ECS service, and remote endpoint nodes. Docker is optional; without it, the graph falls back to host-level connections.

## OpenTelemetry correlation

Import an OTLP JSON trace export without making an OTel collector a Maniac runtime dependency:

```bash
cargo run -p maniac-cli -- otel --file trace-export.json
cargo run -p maniac-cli -- --json otel --file trace-export.json
```

Spans are matched to ECS services and containers when local ECS metadata is available. Unmatched spans remain in the report with their trace identity intact.

For complete product direction, see [`docs/product-spec.md`](docs/product-spec.md).

## Design principles

Maniac is local-first, read-only by default, security-conscious, and designed to answer investigation questions quickly. It must never collect environment variables, credentials, request bodies, or arbitrary file contents by default.

## License

Apache-2.0. See [`LICENSE`](LICENSE).
# maniac
