# syntax=docker/dockerfile:1.26.0

ARG ZIGBUILD_IMAGE=ghcr.io/rust-cross/cargo-zigbuild:0.23.0

FROM --platform=$BUILDPLATFORM ${ZIGBUILD_IMAGE} AS builder
ARG RUST_TARGET
ARG PACKAGE

WORKDIR /src

RUN --mount=type=bind,source=rust-toolchain.toml,target=/src/rust-toolchain.toml,ro \
    --mount=type=cache,id=rustup,target=/usr/local/rustup,sharing=locked \
    test -n "${RUST_TARGET}" || { echo "RUST_TARGET build-arg is required" >&2; exit 1; } \
 && test -n "${PACKAGE}" || { echo "PACKAGE build-arg is required" >&2; exit 1; } \
 && rustup show active-toolchain \
 && rustup target add "${RUST_TARGET}"

RUN --mount=type=bind,source=.,target=/src,ro \
    --mount=type=cache,id=rustup,target=/usr/local/rustup,sharing=locked \
    --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target-${RUST_TARGET},target=/cargo-target,sharing=locked \
    CARGO_TARGET_DIR=/cargo-target \
    cargo zigbuild --release --locked --target "${RUST_TARGET}" -p "${PACKAGE}" \
 && cp "/cargo-target/${RUST_TARGET}/release/${PACKAGE}" /out

FROM scratch

COPY --from=builder --chown=65532:65532 --chmod=755 /out /app

USER 65532:65532

ENV HEALTH_SOCKET=/dev/shm/dka-health.sock
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s --retries=3 \
  CMD ["/app", "health"]

ENTRYPOINT ["/app"]
