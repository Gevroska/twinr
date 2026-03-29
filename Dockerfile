# syntax=docker/dockerfile:1

FROM node:20-slim AS builder
WORKDIR /app

COPY package.json bun.lockb tsconfig.json postcss.config.js tailwind.config.js ./
COPY src ./src
COPY templates ./templates
COPY public ./public
COPY front/package.json front/package-lock.json ./front/
COPY front/src ./front/src
COPY front/assets ./front/assets
COPY front/index.html ./front/index.html
COPY front/postcss.config.js front/tailwind.config.js ./front/

RUN npm install
RUN npm --prefix front ci

# Disable experimental features by default in docker images
RUN echo "VITE_ENABLE_EXPERIMENTAL=false" > front/.env

RUN npm run build:front
RUN npm run build:setup-node

FROM node:20-slim AS runtime
WORKDIR /app

COPY package.json bun.lockb tsconfig.json postcss.config.js tailwind.config.js ./
COPY src ./src
COPY templates ./templates
COPY public ./public
COPY --from=builder /app/build ./build
COPY --from=builder /app/public ./public

RUN npm install --omit=dev

EXPOSE 3000

CMD ["node", "build/src/index.js"]
