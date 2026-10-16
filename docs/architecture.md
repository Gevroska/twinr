# Architecture

Twinr uses a Rust server built with Axum, Tokio, and Reqwest, and a SolidJS web interface. The server serves the built frontend, requests Twitch metadata and media, and proxies playback to the browser. FFmpeg is used for optional Opus audio and VOD downloads. There is no server-side favorites database.

Deployment settings are documented in [Setup](setup.md); request formats are in [API](api.md).

## Source layout

| Location | Responsibility |
| --- | --- |
| `src/main.rs` | Configuration and server startup |
| `src/config.rs`, `src/state.rs`, `src/errors.rs` | Validated settings, shared connection pool and concurrency limits, application errors |
| `src/twitch.rs`, `src/cache.rs`, `src/metadata.rs` | Bounded Twitch requests, metadata caching, API handlers |
| `src/security.rs`, `src/proxy.rs` | Outbound HTTP validation, DNS and redirect checks, streaming responses |
| `src/hls.rs`, `src/media.rs` | Rendition selection, URL resolution, manifest rewriting |
| `src/ffmpeg.rs` | Process limits, private HLS gateway, Opus encoding, VOD remuxing |
| `src/chat.rs` | Anonymous Twitch IRC connection, message parsing, browser WebSocket |
| `src/clips.rs`, `src/routes.rs` | Clip embeds, routing, static files, response headers |
| `front/src/` | SolidJS routes, player, chat rendering, browser favorites |

## Frontend and assets

Vite 8 builds the frontend using Rolldown/Oxc for JavaScript and Lightning CSS for CSS minification, with a Solid 1-compatible Vite plugin. Vite configuration is ESM; PostCSS/Tailwind configuration remains CommonJS. The build copies the frontend output into `public/` for the Rust server to serve.

Secondary routes load on demand. The HLS player is a separate chunk, imported only when needed for playback. Home, offline channels, clips, and direct Opus playback do not load it. A build check caps initial JavaScript at 64 KiB and rejects eager player or secondary-route imports.

Production JavaScript and CSS are minified. Assets use hexadecimal hashes so the Rust server can recognize them and send immutable cache headers; entry HTML revalidates. Deferred chunks use the same static-file server, without server rendering or extra API polling.

Instance settings share one pending browser request and a 60-second, tab-local cache refreshed on demand. Thumbnails load near the viewport. Leaving a page cancels outstanding channel, VOD, and favorites requests and channel retry timers. Leaving native Opus playback closes its media response.

Live chat batches rendering once per animation frame, retains at most 1,000 pending messages while backgrounded, and backs off failed reconnects. VOD chat follows the player's clock. Video playback uses hls.js for adaptive or manual quality selection, with native HLS as a fallback where supported.

Favorites are stored in browser `localStorage`. The server receives channel lookups to populate the favorites page but does not persist the user's favorites list.

## Metadata cache

| Data | TTL |
| --- | --- |
| Live stream and viewer metadata | 5 seconds |
| Clip playback tokens | 30 seconds |
| Lists | 60 seconds |
| Profile, VOD, clip, and emote metadata | 5 minutes |
| Stream and VOD playback access tokens | Not cached |

Concurrent cache misses for the same operation and variables share one pending request. Failures are not retained. Independent requests can run concurrently; lookups that depend on earlier results, such as emotes by channel ID, remain sequential.

The cache defaults to 2,048 entries, with a separate 32 MiB serialized-payload budget and a 256 KiB maximum cached response. JSON and key overhead add to actual heap usage. Upstream bodies and concurrency are also bounded.

Eviction removes the oldest completed entry; abandoned pending entries become evictable after 60 seconds. Each replica has its own cache and limits.

## Media, audio, and downloads

The HLS rewriter resolves relative URLs and preserves nested playlists, alternate audio, keys, initialization maps, byte ranges, muted segment names, and unknown Twitch tags.

Opus encoding selects Twitch's native audio-only rendition first, falling back to a video rendition only when no audio-only variant exists. Hosts explicitly configure the available bitrates.

VOD downloads stream-copy tracks into fragmented MP4 with FFmpeg (`-c copy`). The `aac_adtstoasc` filter converts Twitch's AAC framing without re-encoding. The response starts as output becomes available and uses a sanitized title/ID filename. No full VOD is buffered in server or browser memory, and no temporary VOD file is written. The browser download manager handles progress and cancellation.

Downloads and Opus share the FFmpeg process limit. Exhausted capacity returns HTTP 503 with `Retry-After: 5` instead of building an unbounded queue. Disconnects and failures kill and reap the child before releasing its slot. Startup has a 30-second deadline; stalled output has a 60-second deadline.

FFmpeg receives a per-process loopback gateway URL containing an unpredictable secret. The gateway rewrites nested HLS resources and applies the same host, DNS, and redirect policy as ordinary proxying. FFmpeg cannot use file or HTTPS input protocols directly.

## Network policy and response handling

Ordinary proxy requests accept HTTPS on port 443 for allowed Twitch/CDN domains and explicitly listed Twitch CloudFront distributions. IP literals, URL credentials, unrelated hosts, unsafe redirects, and non-public DNS answers are rejected. DNS checks include mixed public/private answers and IPv6 transition/reserved ranges. Environment HTTP proxies are disabled. New legitimate Twitch CDN distributions may need an allowlist update.

Media responses respect downstream backpressure and preserve Range/Content-Range when supplied upstream. The media proxy rejects HTML/SVG responses. Clip embed fields are escaped. Responses include `nosniff`, `no-referrer`, and permissions-policy headers.

The bundled frontend uses same-origin API requests. Permissive cross-origin CORS is not enabled; external browser clients need a same-origin reverse proxy. `INSTANCE_URL` sets the permitted browser origin for chat behind a reverse proxy. See the [deployment guidance](setup.md#reverse-proxy-and-public-access) for operator responsibilities and [Roadmap](../ROADMAP.md) for limitations and deferred work.
