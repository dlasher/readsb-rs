FROM rust:1-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    libusb-1.0-0-dev && \
    rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libncurses6 libzstd1 ca-certificates librtlsdr0 libusb-1.0-0 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/readsb /usr/local/bin/
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh
EXPOSE 30002 30003 30005
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
