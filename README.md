# Twineo

Twineo is a privacy-focused alternative front-end to Twitch inspired by [Invidious](https://github.com/iv-org/invidious) and [Nitter](https://github.com/zedeus/nitter).

Twineo aims to provide:

- No trackers: all media is requested by the server, reducing IP leakage and fingerprinting.
- Lightweight UI: only the components required for browsing streams/VODs/clips are loaded.
- Open Source: the full source code is AGPL.

---

## Current architecture

- **Backend:** Node.js + TypeScript + Express.
- **Frontend:** SolidJS + Vite + TailwindCSS + daisyUI.
- **Media path:** Twineo obtains access tokens via Twitch GQL and proxies media playlists.

This keeps the stack lightweight while still supporting modern tooling.

---

## Local development

```bash
npm install
npm run build:front
npm run build:setup-node
npm run start
```

Twineo serves on `http://localhost:3000`.

---

## Docker

### Run with `docker run`

Run container:

```bash
docker run --rm -p 3000:3000 \
  -e CLIENTID=kimne78kx3ncx6brgo4mv6wki5h1ko \
  -e USERAGENT="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36" \
  ghcr.io/gevroska/twineo:latest
```

### Run with Docker Compose

```bash
docker compose up --build
```

Compose file uses `image: ghcr.io/gevroska/twineo:latest`, so the generated image is consistently tagged.

### Docker Compose example (explicit)

You can run with an explicit compose file (similar to the command you shared):

```yaml
services:
  twineo:
    image: ghcr.io/gevroska/twineo:latest
    build:
      context: .
      dockerfile: Dockerfile
    container_name: twineo
    ports:
      - "127.0.0.1:3002:3000"
    environment:
      USERAGENT: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:149.0) Gecko/20100101 Firefox/149.0"
    restart: "no"
```

Then start it:

```bash
docker compose up --build -d
```


---

## Environment variables

- `CLIENTID` (optional): Twitch client ID. Defaults to a public web client value.
- `USERAGENT` (optional): User-Agent used for Twitch API/media requests.

---

## Disclaimer

All content on Twineo (including any hosted instance) is served from Twitch infrastructure. Any complaints (DMCA/content removal) should be directed to Twitch. Twineo is not affiliated with Twitch.
