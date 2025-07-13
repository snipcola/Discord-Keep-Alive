ARG NODE_VERSION=24.4.0-alpine

FROM node:$NODE_VERSION AS build
WORKDIR /usr/src/build
COPY package.json ./
RUN npm install --production

FROM node:$NODE_VERSION
USER node
WORKDIR /usr/src/app
COPY --from=build --chown=node:node /usr/src/build/node_modules ./node_modules
COPY --chown=node:node . .
CMD ["node", "."]