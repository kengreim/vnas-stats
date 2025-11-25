# Builder image
FROM rust:1.91.1 AS builder
WORKDIR /app

# Pre-fetch dependencies with cached Cargo metadata
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/
COPY artcc_fetcher/Cargo.toml artcc_fetcher/
RUN cargo fetch

# Copy sources
COPY shared ./shared
COPY artcc_fetcher ./artcc_fetcher

# Build release binary
RUN cargo build --release -p artcc_fetcher

# Runtime image
FROM debian:trixie-slim
RUN apt-get update && apt install -y openssl && apt-get install -y ca-certificates & apt-get install wget && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy binary and migrations
COPY --from=builder /app/target/release/artcc_fetcher /usr/local/bin/artcc_fetcher
COPY artcc_fetcher/migrations ./migrations

ENTRYPOINT ["artcc_fetcher"]
