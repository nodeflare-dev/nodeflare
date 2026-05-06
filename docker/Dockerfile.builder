# Build stage
FROM rust:1.82-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations

RUN cargo build --release --package mcp-builder

# Runtime stage
FROM alpine:3.20

RUN apk add --no-cache ca-certificates curl git

# Install flyctl
RUN curl -L https://fly.io/install.sh | FLYCTL_INSTALL=/usr/local sh

WORKDIR /app

COPY --from=builder /app/target/release/mcp-builder /app/mcp-builder

ENV RUST_LOG=info
ENV PATH="/usr/local/bin:${PATH}"

CMD ["/app/mcp-builder"]
