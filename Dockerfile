FROM rust:1-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --locked
COPY src/ src/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    librtlsdr0 libncurses6 libzstd1 ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/readsb /usr/local/bin/
EXPOSE 30002 30003 30005
ENTRYPOINT ["/usr/local/bin/readsb"]
