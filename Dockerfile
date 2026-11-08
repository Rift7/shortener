# --- build stage ---
FROM rust:1-slim-bookworm AS builder
WORKDIR /src

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
COPY tests ./tests
RUN touch src/main.rs src/lib.rs && cargo build --release

# --- runtime stage ---
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data shortener

COPY --from=builder /src/target/release/shortener /usr/local/bin/shortener

USER shortener
WORKDIR /data
ENV SHORTENER_BIND=0.0.0.0:8080 \
    SHORTENER_DB=/data/shortener.db \
    RUST_LOG=shortener=info,tower_http=info,warn

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/shortener"]
