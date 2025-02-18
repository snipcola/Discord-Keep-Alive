FROM node:23.8.0-alpine
RUN mkdir -p /usr/src/app && chown -R node:node /usr/src/app
WORKDIR /usr/src/app
COPY package.json src LICENSE ./
RUN npm install
USER node
COPY --chown=node:node . .
CMD ["node", "."]