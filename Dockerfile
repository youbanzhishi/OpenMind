# OpenMind - AI-native personal knowledge engine
# Multi-stage build: build → runtime

# === Build Stage ===
FROM rust:1.75-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/openmind-core/Cargo.toml crates/openmind-core/Cargo.toml
COPY crates/openmind-ingest/Cargo.toml crates/openmind-ingest/Cargo.toml
COPY crates/openmind-search/Cargo.toml crates/openmind-search/Cargo.toml
COPY crates/openmind-graph/Cargo.toml crates/openmind-graph/Cargo.toml
COPY crates/openmind-api/Cargo.toml crates/openmind-api/Cargo.toml
COPY crates/openmind-cli/Cargo.toml crates/openmind-cli/Cargo.toml

# Create dummy sources for dependency caching
RUN mkdir -p crates/openmind-core/src && echo "" > crates/openmind-core/src/lib.rs && \
    mkdir -p crates/openmind-ingest/src && echo "" > crates/openmind-ingest/src/lib.rs && \
    mkdir -p crates/openmind-search/src && echo "" > crates/openmind-search/src/lib.rs && \
    mkdir -p crates/openmind-graph/src && echo "" > crates/openmind-graph/src/lib.rs && \
    mkdir -p crates/openmind-api/src && echo "" > crates/openmind-api/src/lib.rs && \
    mkdir -p crates/openmind-cli/src && echo "fn main(){}" > crates/openmind-cli/src/main.rs

RUN cargo build --release 2>/dev/null || true

# Build actual sources
COPY crates/ crates/
RUN cargo build --release --bin openmind

# === Runtime Stage ===
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/openmind /app/openmind
COPY .well-known/ /app/.well-known/

# Qdrant is expected as a sidecar service
# Connect via QDRANT_URL environment variable (default: http://qdrant:6333)
ENV QDRANT_URL=http://qdrant:6333
ENV RUST_LOG=info
ENV PORT=9090

EXPOSE 9090

CMD ["/app/openmind"]
