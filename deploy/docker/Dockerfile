# Stage 1: Build
FROM rust:1.82-bookworm AS builder

RUN apt-get update && apt-get install -y \
    cmake \
    clang \
    libclang-dev \
    librocksdb-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release --bin rustchain --bin rustchain-wallet

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    librocksdb-dev \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r rustchain && useradd -r -g rustchain rustchain

COPY --from=builder /build/target/release/rustchain /usr/local/bin/rustchain
COPY --from=builder /build/target/release/rustchain-wallet /usr/local/bin/rustchain-wallet
COPY --from=builder /build/config /etc/rustchain/config

RUN mkdir -p /var/lib/rustchain /var/log/rustchain \
    && chown -R rustchain:rustchain /var/lib/rustchain /var/log/rustchain

USER rustchain

EXPOSE 30303 8080 9090

VOLUME ["/var/lib/rustchain"]

ENTRYPOINT ["rustchain"]
CMD ["run", "--data-dir", "/var/lib/rustchain", "--config", "/etc/rustchain/config/node.toml"]
