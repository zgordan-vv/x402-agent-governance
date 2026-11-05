FROM rust:1.86-bookworm AS builder
WORKDIR /workspace
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
RUN cargo build --release -p governance-api -p governance-worker

FROM debian:bookworm-slim AS api
RUN useradd --system --uid 10001 governance
COPY --from=builder /workspace/target/release/governance-api /usr/local/bin/governance-api
USER governance
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/governance-api"]

FROM debian:bookworm-slim AS worker
RUN useradd --system --uid 10001 governance
COPY --from=builder /workspace/target/release/governance-worker /usr/local/bin/governance-worker
USER governance
ENTRYPOINT ["/usr/local/bin/governance-worker"]
