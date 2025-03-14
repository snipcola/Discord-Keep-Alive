FROM node:23.9.0-alpine AS build
WORKDIR /usr/src/build
COPY package.json ./
RUN npm install --production

FROM node:23.9.0-alpine
USER node
WORKDIR /usr/src/app
COPY --from=build --chown=node:node /usr/src/build/node_modules ./node_modules
COPY --chown=node:node . .
CMD ["node", "."]