FROM oven/bun:1.2.2-alpine
RUN mkdir -p /usr/src/app && chown -R bun:bun /usr/src/app
WORKDIR /usr/src/app
COPY package.json src LICENSE ./
RUN bun install
USER bun
COPY --chown=bun:bun . .
CMD ["bun", "."]