# Setup

The Docker image includes the server, web interface, and FFmpeg. You do not need a Twitch account, API credentials, or a database. To run from source, see [Contributing](../CONTRIBUTING.md).

## Docker

For a local installation:

```sh
docker run -d --name twinr --restart unless-stopped -p 127.0.0.1:3000:3000 ghcr.io/gevroska/twinr:latest
```

Open [localhost:3000](http://localhost:3000), then enter a channel name, Twitch URL, or VOD ID. This port binding only allows connections from the Docker host.

To stop or remove this container:

```sh
docker stop twinr
docker rm twinr
```

Favorites are saved in browser storage for each instance address, not in the container. Use the Favorites page's export/import controls when changing browsers or instance addresses. No persistent server volume is required.

## Docker Compose

Save this as `compose.yaml` in an empty directory:

```yaml
services:
  twinr:
    image: ghcr.io/gevroska/twinr:latest
    ports:
      - "127.0.0.1:3000:3000"
    restart: unless-stopped
```

From that directory, start Twinr:

```sh
docker compose up -d
```

For access from other devices on your LAN, change the mapping to `"3000:3000"` and open `http://<server-address>:3000`. That mapping listens on all host interfaces; restrict access with your firewall as appropriate.

### Audio options

Native audio-only playback needs no extra configuration. To also offer Opus at selected bitrates, add this under the `twinr` service:

```yaml
    environment:
      OPUS_AUDIO_BITRATES: "32,64,96"
      MAX_FFMPEG_PROCESSES: "4"
```

Run `docker compose up -d` to apply the change. The quality selector will offer these Opus bitrates in kbps. Opus encoding uses server CPU; native audio-only playback does not require transcoding.

Opus and VOD downloads share the process limit. When all slots are occupied, new requests return HTTP 503 with `Retry-After: 5`.

### Updates and logs

For a Compose installation, download the current image and recreate the service:

```sh
docker compose pull
docker compose up -d
docker compose logs --tail=100 twinr
```

For the standalone Docker command, pull the image, stop and remove the old container, then repeat your original run command with the same options:

```sh
docker pull ghcr.io/gevroska/twinr:latest
docker stop twinr
docker rm twinr
```

`latest` follows published builds from `main`. To keep a fixed version, use a published image digest instead.

## Reverse proxy and public access

Put Twinr behind an HTTPS reverse proxy and set `INSTANCE_URL` to the public origin:

```yaml
    environment:
      INSTANCE_URL: "https://twitch.example.com"
```

This sets the clip embed URL base and the allowed browser origin for chat WebSockets. Keep the interface and API on the same origin; browser access from unrelated origins is not enabled.

Configure the reverse proxy to:

- Forward requests to port 3000 and support WebSocket upgrades at `/`.
- Stream media and downloads without response buffering, and allow long-lived responses.
- Avoid logging signed media URLs or their query strings.

The localhost port binding above works for a reverse proxy running on the Docker host. If the proxy runs in another container, connect both services to a shared Docker network and use `twinr:3000` as its upstream instead.

For a public instance, configure authentication or rate limits at the proxy and CPU, memory, and PID limits in the container runtime. Proxying video uses server bandwidth. The FFmpeg process limit controls concurrency, not CPU usage. The image runs as unprivileged UID/GID 65532.

## Downloads

Open a URL like this on your instance, replacing `1234567890` with the VOD ID:

```text
http://localhost:3000/api/vod/1234567890/download?quality=720
```

Omit `quality` for the source/default selection, or use `quality=audio_only` for audio in an MP4 container. Progress and cancellation appear in the browser's download manager.

The download endpoint is available in the standard server. The download button in the web interface is experimental and only appears in a [frontend built with the experimental flag](../CONTRIBUTING.md#experimental-download-button); setting a runtime environment variable on the published image will not enable it.

Downloads begin while the VOD is being fetched. Their final size is unknown in advance, and interrupted downloads cannot be resumed. Source codecs must be MP4-compatible; Twinr does not re-encode incompatible video. See [known limitations](../ROADMAP.md#known-limitations).

## Environment variables

All of these are optional. Docker already includes FFmpeg with Opus support.

| Variable | Default | Meaning |
| --- | --- | --- |
| `CLIENTID` | Built-in public Twitch client ID | Twitch GraphQL client header; not a personal login token |
| `USERAGENT` | Built-in browser user agent | User-Agent header sent upstream |
| `INSTANCE_URL` | Unset | Public origin for chat WebSocket checks and clip embed URLs |
| `OPUS_AUDIO_BITRATES` | Disabled | Comma-separated bitrates from 6 to 256 kbps; duplicates are removed; empty or `no` disables Opus |
| `MAX_FFMPEG_PROCESSES` | `4` | Shared Opus/download process limit, from 1 to 64 |
| `METADATA_CACHE_ENTRIES` | `2048` | Cache entry limit, from 1 to 100000; the payload-byte cap still applies |
| `TWITCH_REQUEST_CONCURRENCY` | `16` | Concurrent GraphQL requests, from 1 to 128; waits at most 2 seconds for capacity |
| `UPSTREAM_CONNECT_TIMEOUT_SECONDS` | `10` | Connection timeout, from 1 to 120 seconds |
| `UPSTREAM_READ_TIMEOUT_SECONDS` | `30` | Per-read timeout, from 1 to 300 seconds; no total timeout on media |

Invalid configuration fails startup. Metadata requests have a 15-second overall deadline. Manifests have a 30-second deadline and an 8 MiB size limit; GraphQL responses are limited to 4 MiB. For cache and proxy details, see [Architecture](architecture.md).

## Migrating an old Deno installation

The former Deno server was removed because its proxy routes did not enforce the outbound network policy. Deno hosting is no longer supported.

Start the Rust server or Docker image, verify it works, point your reverse proxy at it, and stop the old Deno process and its service. Updating the repository alone does not stop an existing Deno installation.
