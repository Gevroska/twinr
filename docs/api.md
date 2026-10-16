# API

Twinr serves its frontend and API from the same origin. The API uses Twitch's upstream services and may change as they do. See [the router](../src/routes.rs) for the complete endpoint list.

## Instance information

`GET /api` returns the server version, API version, and enabled Opus bitrates:

```json
{"version":"0.4.0","api":"v0","opusAudioBitrates":[32,64,96]}
```

`opusAudioBitrates` is empty when Opus is disabled. The version shown here is illustrative.

## Channel lookup

`GET /api/user/{username}` looks up one channel. For a batch, send:

```http
POST /api/users
Content-Type: application/json

{"usernames":["raz404","other_channel"]}
```

The batch accepts at most 100 input names. Names are validated and deduplicated case-insensitively, with up to eight concurrent channel lookups. The response has `data` and `failed` arrays. Each successful item includes its `login`; `failed` contains names that could not be loaded. Results may arrive out of order.

## Playback and downloads

| Endpoint | Purpose |
| --- | --- |
| `GET /api/stream/{username}` | Live playback |
| `GET /api/vod/{id}` | VOD playback |
| `GET /api/vod/{id}/download` | Streaming MP4 download |

Playback endpoints accept a `quality` query parameter. Use a height such as `720`, `audio_only`, or `audio_opus_64` for an enabled Opus bitrate. Omit it for automatic selection.

For downloads, omit `quality` for the source/default selection, or use a height or `audio_only`:

```text
/api/vod/1234567890/download?quality=720
```

Downloads are generated streams with no known final size or HTTP resume support. The browser can start saving before the complete VOD has been fetched. See [download instructions](setup.md#downloads) and [media implementation](architecture.md#media-audio-and-downloads).

## Live chat

Connect to the instance root over WebSocket (`ws://localhost:3000/` locally or `wss://your-instance.example/` over HTTPS). Send `JOIN username` as a text frame. The server replies `OK` after connecting to Twitch, then sends JSON arrays of messages.

Each message has `username`, `display-name`, `color`, `mod`, `subscriber`, `emotes`, `message`, and `fragments`. Fragments contain `text` and optionally `emoteId`. Chat is read-only; raw IRC is not forwarded to the browser. See [chat implementation notes](chat.md).

## Proxy access and errors

`/api/urlproxy`, `/api/proxy`, and `/api/playlist` are restricted to the [outbound network policy](architecture.md#network-policy-and-response-handling). They are not general-purpose URL proxies.

Shared validation and proxy/process handlers use 400/403 for invalid or forbidden requests, 502/504 for upstream failures or timeouts, and 503 when process capacity is exhausted. Some playback/metadata handlers also return 500 or endpoint-specific errors when Twitch data is unavailable.

Browser clients must use the same origin or a same-origin reverse proxy. Configure `INSTANCE_URL` for chat origin checks when hosting behind a reverse proxy.
