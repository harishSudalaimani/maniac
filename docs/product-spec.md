Maniac

Build Maniac, a production-grade open-source infrastructure investigation and live topology tool written primarily in Rust.

1. Product Vision

Maniac answers one question:

“What the hell is happening on my infrastructure right now?”

It is NOT another Prometheus, Grafana, OpenTelemetry collector, or generic system monitor.

Maniac should correlate infrastructure information across:

Cloud / ECS
    ↓
EC2 host
    ↓
Container
    ↓
Process
    ↓
Socket
    ↓
Network connection
    ↓
Remote endpoint

The goal is to let an engineer investigate production infrastructure without SSHing into dozens or hundreds of machines.

Example:

ECS Service: payments
    ↓
Task: 8f71c9
    ↓
EC2: i-0123456789
    ↓
Container: api
    ↓
PID: 18231
    ↓
node server.js
    ↓
10.0.5.23:5432
    ↓
PostgreSQL

Maniac should make this relationship visible.

⸻

2. Core Design Philosophy

Maniac should be:

* Fast
* Lightweight
* Rust-native
* Security-conscious
* Read-only by default
* Zero application instrumentation required
* Useful from the terminal
* Designed for production environments
* Able to work without AWS credentials for host-level inspection
* Able to integrate with cloud/container metadata when credentials are available
* Extensible to Docker, ECS, Kubernetes, containerd, and bare-metal systems

Do NOT turn this into a giant monitoring platform.

The first version should be an excellent live infrastructure investigation tool.

⸻


3. Architecture

Use a collector + central server architecture.

                         Maniac CLI
                             │
                       Maniac Server
                             │
             ┌───────────────┼───────────────┐
             │               │               │
        EC2 Agent       EC2 Agent       EC2 Agent
             │               │               │
           ECS             ECS             ECS
             │               │               │
        containers      containers      containers
             │               │               │
         processes       processes       processes
             │               │               │
         sockets         sockets         sockets

The agent runs on each infrastructure host.

The agent collects local infrastructure information and sends it to the Maniac server.

The CLI connects to the server and provides interactive investigation.

Agents should use outbound connections to the server. Do not require inbound ports on production EC2 hosts.

⸻

4. Components

Create a Rust workspace:

maniac/
├── crates/
│   ├── maniac-agent/
│   ├── maniac-server/
│   ├── maniac-cli/
│   ├── maniac-core/
│   ├── maniac-protocol/
│   ├── maniac-linux/
│   ├── maniac-docker/
│   ├── maniac-ecs/
│   └── maniac-tui/
│
├── docs/
├── examples/
├── tests/
├── Cargo.toml
├── README.md
└── LICENSE

Keep platform-specific functionality isolated.

Linux is the primary target for the initial release.

⸻

5. Host Collector

The agent must inspect the Linux host.

Collect:

System

* hostname
* kernel version
* OS
* architecture
* uptime
* CPU count
* CPU utilization
* memory
* swap
* filesystem usage
* network interfaces
* network throughput

Processes

For each process where permissions allow:

* PID
* PPID
* process name
* executable
* command line
* CPU usage
* memory usage
* start time
* user
* open file descriptors where possible
* open network sockets

Do not expose environment variables by default because they frequently contain secrets.

⸻

6. Network Collector

This is one of the most important components.

Collect:

* listening TCP ports
* listening UDP ports
* established TCP connections
* connection state
* local IP
* local port
* remote IP
* remote port
* PID
* owning process
* protocol
* interface

Build the relationship:

socket
  ↓
PID
  ↓
process
  ↓
container
  ↓
ECS task / Kubernetes pod
  ↓
service

The system must not simply dump netstat output.

It must correlate network information with processes and containers.

⸻

7. Container Discovery

Initially support:

1. Docker
2. ECS on EC2
3. containerd where practical

For every container, collect:

* container ID
* name
* image
* image tag
* status
* created time
* uptime
* CPU
* memory
* network statistics
* filesystem usage where available
* exposed/listening ports
* processes
* PID namespace information
* network namespace information

Build:

Container
    ↓
PID namespace
    ↓
Process
    ↓
Socket
    ↓
Connection

Do not assume container IDs are enough to identify the workload.

⸻

8. ECS Integration

ECS is a first-class integration.

Support ECS EC2 workloads.

Where AWS permissions are available, correlate local container information with ECS metadata:

Cluster
    ↓
Service
    ↓
Task
    ↓
Container
    ↓
Host
    ↓
Process
    ↓
Socket

Expose:

* ECS cluster
* ECS service
* task ARN
* task ID
* task definition
* container name
* image
* desired status
* last status
* host EC2 instance

The tool must still work in degraded mode when AWS APIs are unavailable.

Local host/container information should continue working.

⸻

9. Central Server

Build a lightweight Maniac server.

Responsibilities:

* authenticate agents
* maintain agent connections
* receive snapshots/events
* maintain current infrastructure state
* correlate hosts/containers/processes/network connections
* expose query APIs
* provide streaming updates to CLI clients

Avoid using a heavy database for the MVP.

Use an in-memory state model initially.

Design the interfaces so persistent storage can be added later.

⸻

10. Agent Communication

Use a strongly typed protocol.

Prefer:

* gRPC
* or another efficient bidirectional streaming protocol

Agents should maintain a persistent outbound connection.

The server should be able to request:

snapshot
process details
container details
network details

from an agent.

Agents should periodically publish lightweight updates.

Avoid sending massive snapshots unnecessarily.

⸻

11. CLI

The CLI should support:

maniac

Show the main interactive dashboard.

Also support:

maniac hosts
maniac containers
maniac processes
maniac ports
maniac connections
maniac services
maniac inspect <resource>
maniac top

Machine-readable output:

maniac --json
maniac containers --json
maniac connections --json

Support:

maniac --server https://maniac.example.com

and configuration through a config file/environment variables.

⸻

12. TUI

Use ratatui.

The main screen should look approximately like:

MANIAC
────────────────────────────────────────────────────────────
CLUSTER       production
HOSTS         100
TASKS         487
CONTAINERS    963
PROCESSES     8,421
CONNECTIONS   14,291
CPU            42%
MEMORY         68%
NETWORK        ↓ 4.8 GB/s ↑ 1.2 GB/s
SERVICES
────────────────────────────────────────────────────────────
SERVICE             TASKS     CPU       RAM       NETWORK
payments             42       31%       28GB      1.2GB/s
users                31       18%       19GB      620MB/s
video                86       64%       71GB      2.4GB/s
notifications        17        9%        8GB      120MB/s

Keyboard navigation:

↑ ↓       Navigate
Enter     Inspect
/         Filter
f         Filter
r         Refresh
c         Connections
p         Processes
t         Containers
h         Hosts
q         Quit

Keep the UI fast even with thousands of processes/connections.

⸻

13. Inspection View

When selecting a service:

SERVICE: payments
TASKS
────────────────────────────────────
abc123   i-0123...   api       12% CPU   512MB
def456   i-0192...   api        8% CPU   480MB
ghi789   i-0312...   api       21% CPU   710MB

Selecting a task:

TASK: abc123
ECS SERVICE
payments
EC2
i-0123456789
CONTAINERS
api
nginx
PROCESSES
PID       COMMAND
18231     node server.js
19321     nginx
NETWORK
REMOTE                 STATE
10.0.5.23:5432         ESTABLISHED
10.0.5.30:6379         ESTABLISHED
142.250.x.x:443        ESTABLISHED

Selecting the process:

PROCESS
PID
18231
COMMAND
node server.js
CONTAINER
api
ECS TASK
abc123
LISTENING
0.0.0.0:3000
CONNECTIONS
→ postgres:5432
→ redis:6379
→ external-service:443

⸻

14. Network Topology

This is a key feature.

Represent infrastructure relationships as a graph:

payments
   │
   ├── api
   │    │
   │    ├──── postgres:5432
   │    ├──── redis:6379
   │    └──── stripe:443
   │
   └── nginx
        │
        └──── api:3000

The topology should be queryable.

Example:

maniac connections --service payments

should answer:

What is this service talking to right now?

⸻

15. OpenTelemetry Integration

Do NOT attempt to replace OpenTelemetry.

Treat OTel as an optional data source.

OTel provides:

traces
metrics
logs

Maniac provides:

hosts
containers
processes
sockets
network connections
live topology

Eventually correlate them.

Example:

HTTP Trace
    ↓
payments-api
    ↓
ECS Task abc123
    ↓
Container api
    ↓
PID 18231
    ↓
TCP connection
    ↓
postgres:5432

The goal is to answer both:

“Why is this request slow?”

and:

“What infrastructure is actually involved?”

Do not make OTel a dependency for the MVP.

⸻

16. Security

Security is extremely important.

Default behavior must be read-only.

Never expose secrets by default.

Do not collect:

* environment variable values
* application request bodies
* passwords
* tokens
* private keys
* arbitrary file contents

Unless explicitly enabled by a future feature.

Agent authentication must be designed from the beginning.

Use TLS for agent/server communication.

Use authentication tokens or certificates.

Document required Linux capabilities.

Do not recommend running the entire server as root.

Only the host collector should receive elevated privileges where required.

⸻

17. Performance Requirements

Maniac is intended for production infrastructure.

Target:

* very low CPU usage
* very low memory usage
* no unnecessary network traffic
* no blocking operations in the collector
* efficient incremental updates
* scalable to hundreds/thousands of hosts
* scalable to tens of thousands of containers
* scalable to large numbers of connections

Do not build a polling architecture that constantly executes shell commands such as:

ps
netstat
ss
docker ps
docker inspect

Use native Linux APIs/procfs/netlink where practical.

Use Docker/container APIs rather than repeatedly spawning CLI commands.

⸻

18. CLI Local Mode

The agent should also be usable without a server.

This must work:

sudo maniac local

or:

sudo maniac

when configured for local mode.

This lets a developer install Maniac on a single EC2 instance and immediately investigate it.

⸻

19. Example Use Cases

Production debugging

maniac inspect payments

Find who owns port 8080

maniac ports 8080

Find all external connections

maniac connections --external

Find the busiest containers

maniac top containers

Find processes with many connections

maniac top connections

Inspect one ECS task

maniac inspect task abc123

Find which services communicate with PostgreSQL

maniac connections --remote-port 5432

JSON integration

maniac connections --json

⸻

20. MVP

Do NOT implement everything initially.

Build the MVP in this order:

Phase 1 — Local Linux

Implement:

* process discovery
* CPU/memory
* TCP/UDP sockets
* listening ports
* process ↔ socket correlation
* terminal UI
* JSON output

Command:

sudo maniac

Phase 2 — Containers

Add:

* Docker discovery
* container ↔ PID correlation
* container network statistics
* container TUI

Phase 3 — ECS

Add:

* ECS metadata
* cluster/service/task correlation
* EC2 host correlation
* ECS task inspection

Phase 4 — Multi-host

Add:

* Maniac agent
* Maniac server
* agent authentication
* streaming updates
* multi-host TUI

Phase 5 — Topology

Add:

* service relationships
* container relationships
* network graph
* connection exploration

Phase 6 — OTel

Add optional:

* OTel traces
* trace ↔ task correlation
* trace ↔ process correlation
* trace ↔ network relationship

⸻

21. Engineering Standards

Use idiomatic modern Rust.

Prefer:

* tokio
* ratatui
* crossterm
* clap
* serde
* tracing
* tonic if using gRPC
* strong error types
* structured logging
* unit tests
* integration tests

Avoid unnecessary dependencies.

Keep the code modular.

Use traits/interfaces around:

HostCollector
ProcessCollector
NetworkCollector
ContainerRuntime
CloudProvider
TelemetryProvider

so future support for Kubernetes, GCP, Azure, Podman, etc. can be added without rewriting the core.

⸻

22. Developer Experience

The final project should be installable like:

brew install maniac

and:

curl -fsSL https://... | sh

Provide Linux binaries for:

x86_64
aarch64

Eventually support macOS for local process/network inspection.

The README should contain:

* architecture
* installation
* quick start
* ECS setup
* Docker setup
* security model
* permissions
* screenshots
* examples
* development instructions

⸻

23. Important Product Principle

Do not build a dashboard for the sake of having a dashboard.

Every screen should help answer an infrastructure question.

The primary workflow is:

Something looks wrong
        ↓
Find the service
        ↓
Find the task
        ↓
Find the host
        ↓
Find the container
        ↓
Find the process
        ↓
Find the connection
        ↓
Understand what it is talking to

Maniac should make this investigation extremely fast.

Final Goal

Make Maniac feel like:

htop
+
ss
+
docker stats
+
docker inspect
+
ECS metadata
+
network topology
+
optional OpenTelemetry correlation

but presented as one coherent tool.

The product should feel lightweight and immediate:

Install it. Run it. See your infrastructure. Find what is happening.

Start by implementing Phase 1 only. Do not jump ahead to ECS, Kubernetes, OTel, or the distributed server until the local Linux collector and TUI are solid.
