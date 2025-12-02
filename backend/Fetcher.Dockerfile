FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json --bin datafeed_fetcher

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json --bin datafeed_fetcher

# Build application
COPY . .
RUN cargo build --release -p datafeed_fetcher

FROM debian:trixie-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && apt-get install -y openssl && apt-get install -y wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/datafeed_fetcher /usr/local/bin
ENTRYPOINT ["/usr/local/bin/datafeed_fetcher"]
