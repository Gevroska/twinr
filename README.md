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
docker build -t twinr-rust .
docker run --rm -p 3000:3000 \
  -e CLIENTID=kimne78kx3ncx6brgo4mv6wki5h1ko \
  -e USERAGENT="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36" \
  twinr-rust
```

## Environment variables

- `CLIENTID` (optional)
- `USERAGENT` (optional)
- `INSTANCE_URL` (optional, used for clip embed metadata)
