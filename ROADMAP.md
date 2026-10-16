# Roadmap

These are known limitations and possible follow-up work, not scheduled releases. Use [Issues](https://github.com/Gevroska/twinr/issues) to discuss a concrete change before starting a larger contribution.

## Known limitations

- Twitch's undocumented GraphQL operations and CDN hosts can change independently of Twinr and interrupt playback.
- Generated VOD downloads have no known final size and cannot be resumed or sought over HTTP. Source codecs must be MP4-compatible; failures end the download instead of re-encoding the video.
- Unusual codecs and muted or discontinuous archives need further playback/download validation.
- The HLS rewriter preserves unknown tags but does not fully implement variable substitution or content steering.
- The download button is experimental and requires a frontend build flag. The download endpoint itself is available in the standard server.

## Deferred work

- Share Opus encoders between listeners. New Ogg listeners need valid headers and timestamps, and slow listeners need independent bounded queues. The current [FFmpeg module](src/ffmpeg.rs) is the place to extend this behavior.

See [Contributing](CONTRIBUTING.md) for development and testing, and [Architecture](docs/architecture.md) for the current implementation.
