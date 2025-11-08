# Contributing to orb8

Thank you for your interest in contributing to orb8! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

This project adheres to a Code of Conduct that all contributors are expected to follow. Please read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before contributing.

## How Can I Contribute?

### Reporting Bugs

Before creating a bug report:
- Check the [issue tracker](https://github.com/Ignoramuss/orb8/issues) to avoid duplicates
- Collect relevant information (OS, kernel version, Kubernetes version, logs)

When creating a bug report, include:
- Clear, descriptive title
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, kernel version, K8s version, CUDA version if applicable)
- Relevant logs and error messages
- Screenshots if applicable

Use the bug report template when creating issues.

### Suggesting Features

Feature suggestions are welcome! When proposing a feature:
- Check existing issues and the [ROADMAP.md](ROADMAP.md)
- Explain the use case and why it's valuable
- Provide examples of how it would work
- Consider performance and security implications

Use the feature request template when creating issues.

### Pull Requests

1. **Fork and Clone**
   ```bash
   git clone https://github.com/YOUR_USERNAME/orb8.git
   cd orb8
   ```

2. **Create a Branch**
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/your-bug-fix
   ```

3. **Make Your Changes**
   - Follow the coding standards below
   - Write tests for new functionality
   - Update documentation as needed

4. **Test Your Changes**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

5. **Commit Your Changes**
   ```bash
   git add .
   git commit -m "feat: add GPU memory leak detection"
   ```

   We follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` new feature
   - `fix:` bug fix
   - `docs:` documentation changes
   - `test:` test additions/changes
   - `refactor:` code refactoring
   - `perf:` performance improvements
   - `chore:` maintenance tasks

6. **Push and Create PR**
   ```bash
   git push origin feature/your-feature-name
   ```
   Then create a PR on GitHub using the PR template.

## Development Setup

### Prerequisites

- **Linux**: Kernel 5.8+ with BTF enabled
- **Rust**: 1.75 or later
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **LLVM/Clang**: For eBPF compilation
  ```bash
  # Ubuntu/Debian
  sudo apt install llvm clang

  # Fedora
  sudo dnf install llvm clang
  ```
- **bpf-linker**: For linking eBPF programs
  ```bash
  cargo install bpf-linker
  ```
- **kubectl**: For Kubernetes integration testing
- **kind** or **minikube**: For local K8s cluster

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Build eBPF probes
cargo xtask build-ebpf

# Run tests
cargo test

# Run with sample cluster
cargo run -- trace network --namespace default
```

### Testing

We use multiple testing strategies:

1. **Unit Tests**
   ```bash
   cargo test --lib
   ```

2. **Integration Tests**
   ```bash
   # Requires a running Kubernetes cluster
   cargo test --test integration
   ```

3. **eBPF Tests**
   ```bash
   # Requires root privileges
   sudo cargo test --test ebpf
   ```

4. **Linting**
   ```bash
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

### Code Style

- Follow Rust standard formatting (use `cargo fmt`)
- Run `cargo clippy` and fix all warnings
- Prefer explicit error handling over `unwrap()`/`expect()`
- Use meaningful variable and function names
- Add documentation comments for public APIs

Example:
```rust
/// Traces network flows for a specific pod.
///
/// # Arguments
/// * `pod_name` - Name of the pod to trace
/// * `namespace` - Kubernetes namespace
///
/// # Errors
/// Returns an error if the pod doesn't exist or eBPF probe fails to load
pub fn trace_pod_network(pod_name: &str, namespace: &str) -> Result<NetworkStats> {
    // Implementation
}
```

## Project Structure

```
orb8/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── ebpf/             # eBPF probe definitions
│   ├── k8s/              # Kubernetes API integration
│   ├── metrics/          # Metrics collection and export
│   └── ui/               # TUI components
├── tests/
│   ├── integration/      # Integration tests
│   └── ebpf/            # eBPF-specific tests
├── docs/                 # Documentation
├── deploy/              # Kubernetes manifests
└── examples/            # Example configurations
```

## eBPF Development

When writing eBPF programs:
- Test with multiple kernel versions (5.8+, 5.15+, 6.0+)
- Minimize overhead (use maps efficiently, avoid unnecessary operations)
- Handle edge cases (missing data, unexpected values)
- Document kernel version requirements

## Documentation

- Update README.md for user-facing changes
- Update ARCHITECTURE.md for design changes
- Add inline code comments for complex logic
- Write examples for new features

## Performance Considerations

orb8 is designed for production environments. When contributing:
- Profile performance impact (`cargo bench`)
- Keep overhead <1% CPU per node
- Minimize memory allocations in hot paths
- Use async/await for I/O operations
- Batch operations where possible

## Security

- Never commit secrets or credentials
- Report security vulnerabilities privately to the maintainers
- Validate all user inputs
- Follow least privilege principle for eBPF capabilities

## Communication

- **GitHub Issues**: Bug reports, feature requests
- **Pull Requests**: Code contributions
- **Discussions**: General questions, ideas

## Recognition

Contributors will be recognized in:
- GitHub contributors list
- Release notes for significant contributions
- Project README (for major features)

## License

By contributing to orb8, you agree that your contributions will be licensed under the Apache License 2.0.

## Questions?

Feel free to open a [discussion](https://github.com/Ignoramuss/orb8/discussions) or reach out via GitHub issues.

Thank you for contributing to orb8!
