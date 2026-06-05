# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:latest AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY vendor/ vendor/
# .cargo/config.toml tells cargo to use the vendored sources (no network needed)
COPY .cargo/ .cargo/
RUN cargo build --release --offline

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/aven /usr/local/bin/aven
COPY aven-core/ /app/aven-core/
COPY bin/ /app/bin/
RUN chmod +x /usr/local/bin/aven /app/bin/avencc /app/bin/test-docker && \
    ln -s /app/bin/avencc /usr/local/bin/avencc

# Tell avencc where to find aven-core/ when installed outside the repo tree
ENV AVEN_REPO_ROOT=/app

WORKDIR /work
ENTRYPOINT ["aven"]
CMD []
