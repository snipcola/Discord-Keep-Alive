FROM scratch
ARG RUST_TARGET
ARG PACKAGE
ARG DIST_DIR=dist

COPY --chown=65532:65532 --chmod=755 ${DIST_DIR}/${RUST_TARGET}/${PACKAGE} /app

USER 65532:65532

ENV HEALTH_SOCKET=/dev/shm/dka-health.sock
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s --retries=3 \
  CMD ["/app", "health"]

ENTRYPOINT ["/app"]
