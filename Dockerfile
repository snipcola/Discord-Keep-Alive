# syntax=docker/dockerfile:1.26.0

ARG ZIGBUILD_IMAGE=ghcr.io/rust-cross/cargo-zigbuild:0.23.0
ARG CARGO_CHEF_VERSION=0.1.77
ARG PACKAGE=discord-keep-alive

ARG IMAGE_TITLE=discord-keep-alive
ARG IMAGE_SOURCE=https://code.snipcola.st/snipcola/Discord-Keep-Alive
ARG IMAGE_LICENSE=ISC
ARG VERSION=0.0.0-dev
ARG REVISION=unknown

FROM --platform=$BUILDPLATFORM ${ZIGBUILD_IMAGE} AS chef
ARG CARGO_CHEF_VERSION
WORKDIR /app
COPY rust-toolchain.toml ./
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-chef-target,target=/tmp/cargo-chef-target,sharing=locked \
    rustup show active-toolchain \
 && CARGO_TARGET_DIR=/tmp/cargo-chef-target \
    cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}"

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG RUST_TARGET
ARG PACKAGE

RUN test -n "${RUST_TARGET}" || { echo "RUST_TARGET build-arg is required" >&2; exit 1; } \
 && rustup show active-toolchain \
 && rustup target add "${RUST_TARGET}"

COPY --from=planner /app/recipe.json recipe.json
# id=cargo-target-${RUST_TARGET} must match cache-map (docker-targets.sh).
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target-${RUST_TARGET},target=/app/target,sharing=locked \
    cargo chef cook \
      --release \
      --locked \
      --zigbuild \
      --recipe-path recipe.json \
      --target "${RUST_TARGET}" \
      -p "${PACKAGE}"

COPY . .
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target-${RUST_TARGET},target=/app/target,sharing=locked \
    cargo zigbuild --release --locked --target "${RUST_TARGET}" -p "${PACKAGE}" \
 && cp "/app/target/${RUST_TARGET}/release/${PACKAGE}" /out

FROM scratch

ARG IMAGE_TITLE
ARG IMAGE_SOURCE
ARG IMAGE_LICENSE
ARG VERSION
ARG REVISION

LABEL org.opencontainers.image.source="${IMAGE_SOURCE}" \
      org.opencontainers.image.title="${IMAGE_TITLE}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.licenses="${IMAGE_LICENSE}"

COPY --from=builder --chown=65532:65532 --chmod=755 /out /app

USER 65532:65532

ENV HEALTH_SOCKET=/dev/shm/dka-health.sock
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s --retries=3 \
  CMD ["/app", "health"]

ENTRYPOINT ["/app"]
