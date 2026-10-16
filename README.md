# Twinr

Twinr is a self-hosted web app for watching Twitch streams, past broadcasts, and clips without a Twitch account. Enter a channel name or paste a Twitch link to start watching.

## Why use it?

Twinr keeps the focus on watching and listening, with a simple interface you can run on your own server:

- Watch live streams and past broadcasts with a choice of video quality, plus read-only chat.
- Listen in audio-only mode, with optional low-bitrate Opus audio when enabled by the host.
- Keep favorite channels in your browser, with import and export to move them between devices.
- [Download past broadcasts as MP4](docs/setup.md#downloads) using a direct link.

The server fetches streams from Twitch on your behalf. Favorites stay in your browser instead of a Twitch account; whoever runs the instance still handles your viewing requests.

If you prefer a desktop app that opens streams in an external player, [Streamlink Twitch GUI](https://github.com/streamlink/streamlink-twitch-gui) is another option. Twinr keeps playback in the browser and can be hosted once for use across your devices.

Twinr is for viewing: it does not sign in to Twitch or send chat messages. Playback depends on Twitch and can break when Twitch changes its services.

## Run with Docker

With [Docker](https://docs.docker.com/get-started/get-docker/) installed:

```sh
docker run -d --name twinr --restart unless-stopped -p 127.0.0.1:3000:3000 ghcr.io/gevroska/twinr:latest
```

Open [localhost:3000](http://localhost:3000). No Twitch credentials or database are needed.

See the [setup guide](docs/setup.md) for Docker Compose, remote access, updates, and audio options.

## Project

Twinr is forked from [Twineo](https://codeberg.org/CloudyyUw/twineo), inspired by [Invidious](https://github.com/iv-org/invidious) and [Nitter](https://github.com/zedeus/nitter), and licensed under [AGPL-3.0](LICENSE).

Report bugs in [Issues](https://github.com/Gevroska/twinr/issues). For development, see [Contributing](CONTRIBUTING.md), [Architecture](docs/architecture.md), [API](docs/api.md), and the [Roadmap](ROADMAP.md).
