# syntax=docker/dockerfile:1
#
# BuildKit cache mounts for the cargo registry and target/ dir (persisted by Depot across
# builds, and across a build killed mid-compile) so the heavy dependency compilation isn't
# redone every deploy. The binary is copied out of the (ephemeral) target cache mount in
# the same RUN so it lands in the image layer.

FROM rust:1.82-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --package mcp-builder && \
    cp target/release/mcp-builder /usr/local/bin/mcp-builder

# Runtime stage
FROM alpine:3.20

RUN apk add --no-cache ca-certificates curl git

# Install flyctl
RUN curl -L https://fly.io/install.sh | FLYCTL_INSTALL=/usr/local sh

WORKDIR /app

COPY --from=builder /usr/local/bin/mcp-builder /app/mcp-builder

ENV RUST_LOG=info
ENV PATH="/usr/local/bin:${PATH}"

CMD ["/app/mcp-builder"]
