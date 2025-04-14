FROM rust:1.85.1 AS builder

RUN update-ca-certificates

# Create appuser
ENV USER=fetcher
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /fetcher

COPY ./ .

# We no longer need to use the x86_64-unknown-linux-musl target
RUN cargo build -p datafeed_fetcher --release

FROM debian:bookworm-slim as final

RUN apt-get update && apt install -y openssl && apt install -y ca-certificates

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /fetcher

# Copy our build and settings file
COPY --from=builder /fetcher/target/release/datafeed_fetcher ./
COPY --from=builder /fetcher/Settings.toml ./

# Use an unprivileged user.
USER fetcher:fetcher

CMD ["/fetcher/datafeed_fetcher"]
