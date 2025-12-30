ARG BUN_VERSION=1.3.5-alpine

FROM oven/bun:$BUN_VERSION AS build
WORKDIR /usr/src/build
COPY package.json bun.lock ./
RUN bun ci --production

FROM oven/bun:$BUN_VERSION
WORKDIR /usr/src/app
COPY --from=build /usr/src/build/node_modules ./node_modules
COPY . .
CMD ["bun", "run", "."]
