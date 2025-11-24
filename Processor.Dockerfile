FROM rust:1.91.1 AS builder
WORKDIR /app

# Pre-fetch dependencies using manifest files for cache efficiency
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/
COPY datafeed_processor/Cargo.toml datafeed_processor/
RUN cargo fetch

# Copy sources
COPY shared ./shared
COPY datafeed_processor ./datafeed_processor
COPY Settings.toml ./Settings.toml

# Build release binary
RUN cargo build --release -p datafeed_processor

FROM debian:trixie-slim as final
RUN apt-get update && apt install -y openssl && apt install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy our build and settings file
COPY --from=builder /app/target/release/datafeed_processor ./datafeed_processor
COPY --from=builder /app/Settings.toml ./Settings.toml
COPY --from=builder /app/datafeed_processor/migrations ./migrations

CMD ["/app/datafeed_processor"]
