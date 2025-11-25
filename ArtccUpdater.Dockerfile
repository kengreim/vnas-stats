FROM rust:1.91.1 AS builder
WORKDIR /app

# Pre-fetch dependencies with cached Cargo metadata
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/
COPY artcc_updater/Cargo.toml artcc_updater/
RUN cargo fetch

# Copy sources
COPY shared ./shared
COPY artcc_updater ./artcc_updater

# Build release binary
RUN cargo build --release -p artcc_updater

# Runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy binary and migrations
COPY --from=builder /app/target/release/artcc_updater /usr/local/bin/artcc_updater
COPY --from=builder /app/artcc_updater/migrations ./migrations

ENTRYPOINT ["artcc_updater"]
