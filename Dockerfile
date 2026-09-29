# syntax=docker/dockerfile:1

FROM node:26-bookworm-slim AS assets
WORKDIR /app

COPY package.json package-lock.json* postcss.config.js ./
COPY templates ./templates
COPY public ./public
COPY scripts ./scripts
COPY front/package.json front/package-lock.json ./front/
COPY front/src ./front/src
COPY front/assets ./front/assets
COPY front/index.html ./front/index.html
COPY front/postcss.config.js front/vite.config.mts front/tsconfig.json ./front/

RUN npm ci
RUN node --test front/src/utils/*.test.mjs && npm run build:assets

# Keep the builder on Bookworm so its glibc matches the runtime image.
FROM rust:1.98-slim-bookworm AS builder
WORKDIR /app
RUN rustup component add rustfmt

COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY templates ./templates
COPY public ./public
COPY --from=assets /app/public ./public
COPY package.json ./package.json

RUN cargo fmt --check && cargo test --release --locked && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/twinr /usr/local/bin/twinr
RUN set -eu; \
    ldd /usr/local/bin/twinr > /tmp/twinr-ldd 2>&1 || { cat /tmp/twinr-ldd; exit 1; }; \
    cat /tmp/twinr-ldd; \
    if grep -q 'not found' /tmp/twinr-ldd; then exit 1; fi; \
    rm /tmp/twinr-ldd
COPY --from=builder /app/public ./public
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/package.json ./package.json

EXPOSE 3000
USER 65532:65532
CMD ["/usr/local/bin/twinr"]
