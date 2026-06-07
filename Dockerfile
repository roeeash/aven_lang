# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:latest AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY vendor/ vendor/
# Generate the vendor config inline — not committed to the repo so normal cargo
# invocations outside Docker still use crates.io.
RUN mkdir -p .cargo && printf '[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "vendor"\n' > .cargo/config.toml
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
