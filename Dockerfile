# Stage 1: Build
FROM rust:1.82-slim AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Pre-build the dependency graph with a dummy main so the Docker layer cache
# can be reused across source-only changes. If dependency resolution fails
# here, we want the build to fail loudly instead of silently swallowing
# the error with `|| true`.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

COPY src/ src/
# `touch` ensures cargo invalidates the dummy compilation above.
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/tgstate /app/tgstate
COPY app/ /app/app/

RUN adduser --disabled-password --gecos "" --uid 10001 appuser && \
    mkdir -p /app/data && \
    chown -R appuser:appuser /app

USER appuser

ENV DATA_DIR=/app/data

EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8000/api/health || exit 1
CMD ["./tgstate"]
