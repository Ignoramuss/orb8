# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1] - 2024-11-27

### Added
- Initial release to crates.io
- `orb8` - Root library crate with re-exports
- `orb8-common` - Shared types for eBPF/userspace communication
- `orb8-cli` - CLI command definitions
- `orb8-agent` - Node agent with eBPF probe loading and ring buffer support

### eBPF Infrastructure (Phase 1)
- Hello World eBPF probe using aya-bpf
- Probe loader with lifecycle management
- Ring buffer for kernel-userspace event communication
- Support for tc classifier attachment

### Infrastructure
- GitHub Actions CI pipeline (check, test, fmt, clippy, eBPF build)
- Lima VM development environment for macOS
- Automated crates.io publishing workflow

---

## Release Notes Format

Each release will include:
- **Added**: New features
- **Changed**: Changes to existing functionality
- **Deprecated**: Features to be removed in future releases
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Security vulnerability fixes

[Unreleased]: https://github.com/Ignoramuss/orb8/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/Ignoramuss/orb8/releases/tag/v0.0.1
