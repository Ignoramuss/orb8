# Full multi-stage build (used in CI or when no local build is available)
FROM rust:1.94-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install nightly \
    && rustup component add rust-src --toolchain nightly

RUN cargo install bpf-linker

WORKDIR /build
COPY . .

RUN cargo build --release -p orb8-agent

FROM debian:bookworm-slim AS release

RUN apt-get update && apt-get install -y --no-install-recommends \
    libelf1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/orb8-agent /usr/local/bin/orb8-agent

ENTRYPOINT ["orb8-agent"]

# Fast local build: copy pre-built binary (use with --target=local)
FROM debian:bookworm-slim AS local

RUN apt-get update && apt-get install -y --no-install-recommends \
    libelf1 \
    && rm -rf /var/lib/apt/lists/*

COPY target/release/orb8-agent /usr/local/bin/orb8-agent

ENTRYPOINT ["orb8-agent"]
