# Stage 1: Build the release binary (fully static via musl)
FROM rust:1.85-alpine AS builder

RUN apk add --no-cache musl-dev pkgconf perl make

WORKDIR /build

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY apps/core/Cargo.toml apps/core/Cargo.toml
COPY packages/types/Cargo.toml packages/types/Cargo.toml
COPY packages/traits/Cargo.toml packages/traits/Cargo.toml
COPY packages/crypto/Cargo.toml packages/crypto/Cargo.toml
COPY packages/plugin-sdk-rs/Cargo.toml packages/plugin-sdk-rs/Cargo.toml
COPY packages/storage-sqlite/Cargo.toml packages/storage-sqlite/Cargo.toml
COPY packages/auth/Cargo.toml packages/auth/Cargo.toml
COPY packages/workflow-engine/Cargo.toml packages/workflow-engine/Cargo.toml
COPY packages/plugin-system/Cargo.toml packages/plugin-system/Cargo.toml
COPY packages/transport-rest/Cargo.toml packages/transport-rest/Cargo.toml
COPY packages/transport-graphql/Cargo.toml packages/transport-graphql/Cargo.toml
COPY packages/transport-caldav/Cargo.toml packages/transport-caldav/Cargo.toml
COPY packages/transport-carddav/Cargo.toml packages/transport-carddav/Cargo.toml
COPY packages/transport-webhook/Cargo.toml packages/transport-webhook/Cargo.toml
COPY packages/dav-utils/Cargo.toml packages/dav-utils/Cargo.toml
COPY packages/test-utils/Cargo.toml packages/test-utils/Cargo.toml
COPY packages/test-fixtures/Cargo.toml packages/test-fixtures/Cargo.toml
COPY plugins/engine/ plugins/engine/

# Create stub lib/main files so cargo can resolve the workspace and cache deps
RUN mkdir -p apps/core/src && echo "fn main() {}" > apps/core/src/main.rs \
    && mkdir -p packages/types/src && echo "" > packages/types/src/lib.rs \
    && mkdir -p packages/traits/src && echo "" > packages/traits/src/lib.rs \
    && mkdir -p packages/crypto/src && echo "" > packages/crypto/src/lib.rs \
    && mkdir -p packages/plugin-sdk-rs/src && echo "" > packages/plugin-sdk-rs/src/lib.rs \
    && mkdir -p packages/storage-sqlite/src && echo "" > packages/storage-sqlite/src/lib.rs \
    && mkdir -p packages/auth/src && echo "" > packages/auth/src/lib.rs \
    && mkdir -p packages/workflow-engine/src && echo "" > packages/workflow-engine/src/lib.rs \
    && mkdir -p packages/plugin-system/src && echo "" > packages/plugin-system/src/lib.rs \
    && mkdir -p packages/transport-rest/src && echo "" > packages/transport-rest/src/lib.rs \
    && mkdir -p packages/transport-graphql/src && echo "" > packages/transport-graphql/src/lib.rs \
    && mkdir -p packages/transport-caldav/src && echo "" > packages/transport-caldav/src/lib.rs \
    && mkdir -p packages/transport-carddav/src && echo "" > packages/transport-carddav/src/lib.rs \
    && mkdir -p packages/transport-webhook/src && echo "" > packages/transport-webhook/src/lib.rs \
    && mkdir -p packages/dav-utils/src && echo "" > packages/dav-utils/src/lib.rs \
    && mkdir -p packages/test-utils/src && echo "" > packages/test-utils/src/lib.rs \
    && mkdir -p packages/test-fixtures/src && echo "" > packages/test-fixtures/src/lib.rs

RUN cargo build --release --package life-engine-core 2>/dev/null || true

# Copy full source and rebuild
COPY apps/core/ apps/core/
COPY packages/ packages/
COPY docs/schemas/ docs/schemas/

RUN cargo build --release --package life-engine-core

# Stage 2: Minimal runtime image
FROM alpine:3.20

RUN apk add --no-cache ca-certificates wget \
    && addgroup -S life-engine \
    && adduser -S life-engine -G life-engine

WORKDIR /app

COPY --from=builder /build/target/release/life-engine-core /usr/local/bin/life-engine-core

RUN mkdir -p /data /plugins /workflows \
    && chown -R life-engine:life-engine /data /plugins /workflows /app

USER life-engine

ENV LIFE_ENGINE_CORE_HOST=0.0.0.0

EXPOSE 3750

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD wget --spider -q http://localhost:3750/api/system/health || exit 1

ENTRYPOINT ["/usr/local/bin/life-engine-core"]
