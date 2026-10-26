FROM scratch
ARG BINARY

COPY --chown=65532:65532 --chmod=755 ${BINARY} /app
COPY --chmod=444 LICENSE NOTICE /licenses/

USER 65532:65532

ENV HEALTH_SOCKET=/dev/shm/app-health.sock
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s --retries=3 \
  CMD ["/app", "health"]

ENTRYPOINT ["/app"]
