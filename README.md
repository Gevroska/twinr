# Twinr

Twinr is a privacy-focused alternative front-end to Twitch inspired by [Invidious](https://github.com/iv-org/invidious) and [Nitter](https://github.com/zedeus/nitter).

## Architecture

- **Backend:** Rust (Axum + Tokio + Reqwest)
- **Frontend:** SolidJS + Vite + TailwindCSS + daisyUI
- **Media path:** The Rust server requests Twitch GQL/media and proxies playlists/media to clients.

## Local development

```bash
npm ci
npm run build:assets
cargo run
```

Server runs on `http://localhost:3000`.

## Docker

```bash
docker pull ghcr.io/gevroska/twinr:latest
docker run --rm -p 3000:3000 \
  -e CLIENTID=kimne78kx3ncx6brgo4mv6wki5h1ko \
  -e USERAGENT="Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:149.0) Gecko/20100101 Firefox/149.0" \
  ghcr.io/gevroska/twinr:latest
```

## Environment variables

- `CLIENTID` (optional)
- `USERAGENT` (optional)
- `INSTANCE_URL` (optional, used for clip embed metadata)


## Docker Compose

```yaml
services:
  twinr:
    image: ghcr.io/gevroska/twinr:latest
    ports:
      - "3000:3000"
    environment:
      - CLIENTID=kimne78kx3ncx6brgo4mv6wki5h1ko
      - USERAGENT=Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:149.0) Gecko/20100101 Firefox/149.0
    restart: unless-stopped
```
