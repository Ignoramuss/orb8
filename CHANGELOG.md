# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.6] - 2026-03-26

### Added
- Test infrastructure: `make smoke-test` (6 assertions), `make e2e-test` (9 assertions across 3 network modes: hostNetwork, regular pods, Service ClusterIP)
- Dockerfile with multi-stage (CI) and local (fast) build targets
- DaemonSet manifest with RBAC (ServiceAccount + ClusterRole for pod list/watch)
- Kind cluster config and e2e test pods (echo-server + traffic-gen with nodeSelector for cross-node traffic)
- Root `orb8` crate restored as install wrapper (`cargo install orb8` now works)
- `pub async fn run()` in orb8-cli for programmatic use

### Fixed
- Release workflow now publishes root `orb8` crate to crates.io (was missing from publish list)

### Changed
- README rewritten to reflect current v0.0.6 capabilities, honest comparison tables, and full roadmap
- CHANGELOG backfilled for v0.0.2 through v0.0.5
- Restored TUI Dashboard, Standalone Mode, DNS Tracing to roadmap (previously dropped)
- ROADMAP.md Phase 3.5 marked complete

## [0.0.5] - 2026-03-25

### Added
- Root `orb8` crate restored as thin wrapper around `orb8-cli`
- `cargo install orb8` now installs the CLI binary

### Changed
- CLI logic moved from orb8-cli/src/main.rs to lib.rs (exposes `pub async fn run()`)

## [0.0.4] - 2026-03-25

### Fixed
- Aggregator enrichment bug: `orb8 flows` returned "unknown/cgroup-0" for all flows because the aggregator used cgroup-based lookup (always returns 0 for TC classifiers). Now uses single IP-based enrichment path for both aggregator and gRPC streams.

### Added
- `orb8-agent/src/net.rs`: consolidated IP formatting, parsing, self-traffic filtering
- Unit tests for aggregator (5 tests) and pod_cache (3 tests)

### Removed
- Dead `src/` directory (18 stub files from pre-workspace era)
- Converted root Cargo.toml to virtual workspace
- Removed unused EnrichedEvent type

### Changed
- Ungated aggregator and pod_cache from `cfg(linux)` (pure data structures, testable on macOS)

## [0.0.3] - 2026-01-07

### Added
- Ring buffer drop counter (EVENTS_DROPPED eBPF map, surfaced in GetStatus RPC)
- Little-endian compile-time assertion (prevents silent data corruption on big-endian)
- Self-traffic filter (agent's own gRPC port excluded from captures)
- IP-based pod enrichment as primary path for TC classifiers

## [0.0.2] - 2025-12-06

### Added
- Network Tracing MVP: eBPF TC classifiers (ingress/egress) with IPv4 5-tuple extraction
- gRPC API: QueryFlows, StreamEvents, GetStatus on port 9090
- CLI commands: `orb8 status`, `orb8 flows`, `orb8 trace network`
- Kubernetes pod watcher with IP-based and cgroup-based cache
- Flow aggregation with 30-second expiration
- Smart interface discovery (eth0, cni0, docker0, br-*)

## [0.0.1] - 2024-11-27

### Added
- Initial release to crates.io
- `orb8-common` — shared types for eBPF/userspace communication
- `orb8-cli` — CLI command definitions
- `orb8-agent` — node agent with eBPF probe loading and ring buffer support
- Hello World eBPF probe using aya-bpf
- GitHub Actions CI pipeline (check, test, fmt, clippy, eBPF build)
- Lima VM development environment for macOS
- Automated crates.io publishing workflow

[0.0.6]: https://github.com/Ignoramuss/orb8/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/Ignoramuss/orb8/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/Ignoramuss/orb8/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/Ignoramuss/orb8/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/Ignoramuss/orb8/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/Ignoramuss/orb8/releases/tag/v0.0.1
