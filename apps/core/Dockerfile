# Stage 1: Build the release binary
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev pkgconf

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY apps/core/ apps/core/
COPY packages/types/ packages/types/
COPY packages/plugin-sdk-rs/ packages/plugin-sdk-rs/
COPY packages/test-utils/ packages/test-utils/
COPY plugins/engine/ plugins/engine/
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

CMD ["/usr/local/bin/life-engine-core"]
