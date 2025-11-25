FROM rust:1.91.1 AS builder
WORKDIR /app

# Pre-fetch dependencies with cached Cargo metadata. We need member manifests and source
# trees so Cargo can discover targets.
COPY Cargo.toml Cargo.lock ./
COPY datafeed_fetcher ./datafeed_fetcher
COPY datafeed_processor ./datafeed_processor
COPY shared ./shared
COPY artcc_updater ./artcc_updater
RUN cargo fetch

# Build release binary
RUN cargo build --release -p artcc_updater

# Runtime image
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates && apt-get install -y openssl && apt-get install -y wget && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy binary and migrations
COPY --from=builder /app/target/release/artcc_updater /usr/local/bin/artcc_updater
COPY --from=builder /app/artcc_updater/migrations ./migrations

ENTRYPOINT ["artcc_updater"]
