FROM node:23.3.0-alpine
RUN mkdir -p /usr/src/app && chown -R node:node /usr/src/app
WORKDIR /usr/src/app
COPY package.json src LICENSE .
RUN apk add --no-cache python3 make g++
RUN npm install -g pnpm && pnpm install
USER node
COPY --chown=node:node . .
CMD ["pnpm", "start"]