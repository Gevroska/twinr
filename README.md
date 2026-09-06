# Twinr

Twinr is a privacy-focused alternative frontend to Twitch, inspired by [Invidious](https://github.com/iv-org/invidious) and [Nitter](https://github.com/zedeus/nitter). Forked from [Twineo](https://codeberg.org/CloudyyUw/twineo).

## Architecture

The production server is Rust (Axum, Tokio, Reqwest); SolidJS owns the UI. Legacy TypeScript server files are not part of the Docker runtime.

- `main.rs`: configuration and server startup.
- `config.rs`, `state.rs`, `errors.rs`: validated configuration, shared connection pool/semaphores and application errors.
- `twitch.rs`, `cache.rs`, `metadata.rs`: bounded Twitch requests, single-flight metadata cache and API handlers. Independent requests run concurrently; dependent requests such as emotes by channel ID remain sequential.
- `security.rs`, `proxy.rs`: validated outbound HTTP, DNS/redirect checks and streaming media responses.
- `hls.rs`, `media.rs`: rendition selection, URL resolution and manifest rewriting. Unknown Twitch tags are retained. Nested playlists, alternate audio, keys, init maps, byte ranges and muted segment names are preserved.
- `ffmpeg.rs`: process limit, private validated HLS gateway, Opus encoding and streaming VOD remuxing.
- `chat.rs`: anonymous Twitch IRC connection, tag/message/emote parsing, JSON WebSocket messages.
- `clips.rs`, `routes.rs`: clip embeds, router, static assets and response headers.

Normal video playback still uses hls.js adaptive/manual quality switching. VOD chat is synchronized using the browser player's clock. Favorites remain in browser localStorage; the server never persists a user's favorites.

### Metadata caching

Live stream/viewer metadata uses a 5-second TTL; clip playback tokens 30 seconds; lists 60 seconds; profile, VOD, clip and emote metadata 5 minutes. Playback access tokens for streams/VODs are not cached. Cache misses for the same operation and variables share one pending request. Failures are not retained.

The cache defaults to 2,048 entries, with a separate 32 MiB serialized-payload budget and 256 KiB maximum cached response. Actual heap usage includes JSON/key overhead. Upstream bodies and concurrency are also bounded. Eviction removes the oldest completed entry; abandoned pending entries become evictable after 60 seconds. Each replica has its own cache and limits.

### Audio and downloads

Opus selects Twitch's native audio-only rendition before starting FFmpeg; it falls back to a video-containing rendition only when no audio-only variant exists. Available bitrates are explicitly configured.

VOD downloads use `GET /api/vod/:id/download?quality=720` (also source/default or `audio_only`). FFmpeg stream-copies tracks into a fragmented MP4 (`-c copy`). The response starts as soon as output is available and includes a sanitized title/ID filename. No complete VOD is buffered in server or browser RAM, and no temporary VOD file is written. Progress/cancellation belongs to the browser download manager. Existing download UI availability remains unchanged.

Downloads share the FFmpeg limit with Opus. Capacity exhaustion returns HTTP 503 with `Retry-After: 5`; the server does not create an unbounded queue. Disconnects/failures kill and reap the child before releasing its permit. Startup has a 30-second deadline and output stalls have a 60-second deadline. Reverse proxies should disable response buffering and permit long-lived streaming responses.

FFmpeg receives only a per-process loopback gateway URL with an unpredictable secret. The gateway rewrites nested HLS resources and applies the same host, DNS and redirect policy as ordinary proxying. FFmpeg cannot use file or HTTPS input protocols directly.

Shared Opus encoders are intentionally deferred. New Ogg listeners need valid headers/timestamps, and slow listeners require independent bounded queues. The centralized output/process module provides a place to implement this without changing normal video playback.

## API changes

- `POST /api/users`, JSON `{"usernames":["raz404","other_channel"]}`: at most 100 input names, validated and case-insensitively deduplicated, with up to 8 concurrent channel lookups. Returns `{"data":[...],"failed":[...]}`. Each item includes `login`; results may arrive out of order.
- `GET /api/vod/:id/download`: progressive MP4 download.
- WebSocket `/`: send `JOIN username`, receive the existing `OK` acknowledgement followed by JSON arrays of messages containing `username`, `display-name`, `color`, `mod`, `subscriber`, `emotes`, `message` and `fragments`. Raw IRC is no longer forwarded.
- Proxy endpoints retain their paths, but reject arbitrary Internet URLs. Validation failures return 400/403; upstream/process failures return 502/504, and exhausted process capacity returns 503.

## Security and deployment

Normal proxying accepts HTTPS on port 443 only, for Twitch/Twitch CDN domains and explicitly listed Twitch CloudFront distributions. IP literals, credentials in URLs, unrelated hosts, unsafe redirects and non-public DNS answers are rejected. DNS answers are checked before connecting, including mixed public/private answers and IPv6 transition/reserved ranges. Environment HTTP proxies are deliberately disabled. New legitimate Twitch CDN distributions may require an explicit allowlist update.

Media responses remain backpressure-aware and preserve Range/Content-Range where supplied upstream. HTML/SVG responses cannot be served through the media proxy. Clip embed fields are escaped. Responses use nosniff, no-referrer and permissions-policy headers. Hashed Vite assets receive immutable caching; entry HTML revalidates.

Permissive cross-origin CORS is removed; the bundled same-origin frontend works without CORS. External API clients that relied on browser cross-origin access will need a same-origin reverse proxy. Configure `INSTANCE_URL` to the public origin for strict WebSocket origin checking behind a reverse proxy.

The Docker runtime runs as unprivileged UID/GID 65532. Keep authentication/rate limits and container CPU/memory/PID limits at your ingress/runtime, particularly for public deployments: Twitch-only proxying still consumes bandwidth, and FFmpeg concurrency is not a CPU quota. Media signed URLs must not be logged by the reverse proxy.

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `CLIENTID` | Built-in public Twitch client ID | Twitch GQL client header |
| `USERAGENT` | Built-in browser user agent | Upstream user-agent header |
| `INSTANCE_URL` | Unset | Public origin and clip embed metadata base |
| `OPUS_AUDIO_BITRATES` | Disabled | Comma-separated unique bitrates, 6–256 kbps; empty or `no` disables |
| `MAX_FFMPEG_PROCESSES` | `4` | Shared Opus/download process limit, 1–64 |
| `METADATA_CACHE_ENTRIES` | `2048` | Cache entry limit, 1–100000; payload-byte cap still applies |
| `TWITCH_REQUEST_CONCURRENCY` | `16` | Concurrent GQL requests, 1–128; at most 2 seconds waiting |
| `UPSTREAM_CONNECT_TIMEOUT_SECONDS` | `10` | Connection timeout, 1–120 seconds |
| `UPSTREAM_READ_TIMEOUT_SECONDS` | `30` | Per-read timeout, 1–300 seconds; no total timeout on media |

Invalid numeric/configuration values fail startup. Metadata requests also have a 15-second overall deadline; manifests have a 30-second deadline and 8 MiB size limit. GQL responses are limited to 4 MiB.

## Development and validation

Rust 1.87+, Node 20+, and FFmpeg with libopus are required.

```bash
npm ci
npm run build:assets
cargo fmt --check
cargo test --locked
node --test front/src/utils/*.test.mjs
npm --prefix front run build
cargo build --release --locked
cargo run
```

The server listens on `http://localhost:3000`. Rust tests cover audio rendition selection, HLS rewriting/relative URLs, SSRF/DNS/redirect validation, IRC/emotes, cache coalescing/expiry/capacity, configuration, process cleanup and delivery before upstream completion. The ignored child fixture is launched by the cleanup test; it is not a skipped behavior test. Frontend tests cover structured chat and favorites validation.

```bash
docker build -t twinr .
docker run --rm -p 3000:3000 \
  -e INSTANCE_URL=https://tw.example.com \
  -e OPUS_AUDIO_BITRATES=32,64,96 \
  -e MAX_FFMPEG_PROCESSES=4 \
  twinr
```

The Docker build runs frontend tests/build, Rust formatting checks, release tests and release build before publishing. Published image: `ghcr.io/gevroska/twinr:latest`.

### Remaining limitations

Runtime frontend packages (including Axios and SolidJS) and compatible build dependencies were updated during the security pass; unused DOMPurify was removed. The remaining npm audit findings concern the legacy Vite/esbuild development server toolchain, which is not shipped in the Rust runtime. A separate Vite major-version migration is recommended; do not expose the development server publicly.

Downloads are generated streams: their final size is unknown beforehand and HTTP resume/seek of the generated file is not supported. Source codecs must be MP4-compatible; failures terminate the download rather than silently re-encoding an entire VOD. Validate unusual codecs and muted/discontinuous archives before long downloads. The HLS rewriter deliberately preserves unknown tags; full variable substitution/content-steering support is not implemented. Twitch's undocumented GQL operations and CDN hosts can change independently of Twinr.
