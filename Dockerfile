# syntax=docker/dockerfile:1.26.0

ARG ZIGBUILD_IMAGE=ghcr.io/rust-cross/cargo-zigbuild:0.23.0

FROM --platform=$BUILDPLATFORM ${ZIGBUILD_IMAGE} AS builder

ARG TARGETPLATFORM
ARG TARGETARCH
WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry-${TARGETARCH} \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git-${TARGETARCH} \
    --mount=type=cache,target=/usr/local/rustup,id=rustup-${TARGETARCH} \
    --mount=type=cache,target=/app/target,id=cargo-target-${TARGETARCH} \
    case "${TARGETPLATFORM}" in \
      linux/amd64) RUST_TARGET=x86_64-unknown-linux-musl ;; \
      linux/arm64) RUST_TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "Unsupported platform: ${TARGETPLATFORM}" >&2; exit 1 ;; \
    esac \
    && rustup show active-toolchain \
    && rustup target add "${RUST_TARGET}" \
    && cargo zigbuild --release --locked --target "${RUST_TARGET}" -p discord-keep-alive \
    && cp "/app/target/${RUST_TARGET}/release/discord-keep-alive" /app/discord-keep-alive

FROM scratch

ARG VERSION=0.0.0-dev
ARG REVISION=unknown

LABEL org.opencontainers.image.source="https://code.snipcola.st/snipcola/Discord-Keep-Alive" \
      org.opencontainers.image.title="discord-keep-alive" \
      org.opencontainers.image.licenses="ISC" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}"

COPY --from=builder --chown=65532:65532 /app/discord-keep-alive /discord-keep-alive

USER 65532:65532

ENV HEALTH_SOCKET=/dev/shm/dka-health.sock
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s --retries=3 \
  CMD ["/discord-keep-alive", "health"]

ENTRYPOINT ["/discord-keep-alive"]
