# Stage 1: Build the release binary
FROM rust:1.85-alpine AS builder

RUN apk add --no-cache musl-dev pkgconf

WORKDIR /build

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY apps/core/Cargo.toml apps/core/Cargo.toml
COPY packages/types/Cargo.toml packages/types/Cargo.toml
COPY packages/plugin-sdk-rs/Cargo.toml packages/plugin-sdk-rs/Cargo.toml
COPY packages/test-utils/Cargo.toml packages/test-utils/Cargo.toml
COPY plugins/engine/ plugins/engine/

# Create stub lib/main files so cargo can resolve the workspace and cache deps
RUN mkdir -p apps/core/src && echo "fn main() {}" > apps/core/src/main.rs \
    && mkdir -p packages/types/src && echo "" > packages/types/src/lib.rs \
    && mkdir -p packages/plugin-sdk-rs/src && echo "" > packages/plugin-sdk-rs/src/lib.rs \
    && mkdir -p packages/test-utils/src && echo "" > packages/test-utils/src/lib.rs

RUN cargo build --release --package life-engine-core || true

# Copy full source and rebuild
COPY apps/core/ apps/core/
COPY packages/types/ packages/types/
COPY packages/plugin-sdk-rs/ packages/plugin-sdk-rs/
COPY packages/test-utils/ packages/test-utils/
COPY docs/schemas/ docs/schemas/

RUN cargo build --release --package life-engine-core

# Stage 2: Minimal runtime image
FROM alpine:3.20

RUN apk add --no-cache ca-certificates wget \
    && addgroup -S life-engine \
    && adduser -S life-engine -G life-engine

COPY --from=builder /build/target/release/life-engine-core /usr/local/bin/life-engine-core

RUN mkdir -p /data && chown life-engine:life-engine /data

USER life-engine

ENV LIFE_ENGINE_CORE_HOST=0.0.0.0

EXPOSE 3750

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD wget --spider -q http://localhost:3750/api/system/health || exit 1

CMD ["/usr/local/bin/life-engine-core"]
