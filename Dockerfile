# syntax=docker/dockerfile:1

FROM node:20-slim AS assets
WORKDIR /app

COPY package.json package-lock.json* postcss.config.js tailwind.config.js ./
COPY templates ./templates
COPY public ./public
COPY scripts ./scripts
COPY front/package.json front/package-lock.json ./front/
COPY front/src ./front/src
COPY front/assets ./front/assets
COPY front/index.html ./front/index.html
COPY front/postcss.config.js front/tailwind.config.js front/vite.config.ts front/tsconfig.json ./front/

RUN npm ci
RUN npm run build:assets

FROM rust:1.87-slim AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY templates ./templates
COPY public ./public
COPY --from=assets /app/public ./public
COPY package.json ./package.json

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/twineo /usr/local/bin/twineo
COPY --from=builder /app/public ./public
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/package.json ./package.json

EXPOSE 3000
CMD ["/usr/local/bin/twineo"]
