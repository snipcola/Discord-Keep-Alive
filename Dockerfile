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
    rustup show active-toolchain \
 && cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}"

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETPLATFORM
ARG TARGETARCH
ARG PACKAGE

RUN case "${TARGETPLATFORM}" in \
      linux/amd64) rust_target=x86_64-unknown-linux-musl ;; \
      linux/arm64) rust_target=aarch64-unknown-linux-musl ;; \
      *) echo "Unsupported platform: ${TARGETPLATFORM}" >&2; exit 1 ;; \
    esac \
 && printf '%s\n' "${rust_target}" >/tmp/rust-target \
 && rustup show active-toolchain \
 && rustup target add "${rust_target}"

COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target-${TARGETARCH},target=/app/target,sharing=locked \
    rust_target="$(cat /tmp/rust-target)" \
 && cargo chef cook \
      --release \
      --locked \
      --zigbuild \
      --recipe-path recipe.json \
      --target "${rust_target}" \
      -p "${PACKAGE}"

COPY . .
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target-${TARGETARCH},target=/app/target,sharing=locked \
    rust_target="$(cat /tmp/rust-target)" \
 && cargo zigbuild --release --locked --target "${rust_target}" -p "${PACKAGE}" \
 && cp "/app/target/${rust_target}/release/${PACKAGE}" /out

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

COPY --from=builder --chown=65532:65532 /out /app

USER 65532:65532

ENV HEALTH_SOCKET=/dev/shm/dka-health.sock
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s --retries=3 \
  CMD ["/app", "health"]

ENTRYPOINT ["/app"]
