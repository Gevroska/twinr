# Contributing

Bug reports and focused pull requests are welcome. Include what you expected, what happened, and steps to reproduce. For playback issues, include the browser, Twinr version or image tag, and whether the problem affects live streams, VODs, or clips. Remove signed media URLs and credentials from logs before posting them.

See [Architecture](docs/architecture.md) for the source layout and [Roadmap](ROADMAP.md) for known limitations and deferred work.

## Local development

Use Node 24 or 26, a current stable Rust toolchain, and FFmpeg with libopus on `PATH`. The [Dockerfile](Dockerfile) records the toolchain versions used by CI.

```sh
git clone https://github.com/Gevroska/twinr.git
cd twinr
npm ci
npm run build:assets
cargo run
```

Open [localhost:3000](http://localhost:3000). `build:assets` installs the frontend dependencies, builds the interface and CSS, and copies the output into `public/`. Run it again after changing frontend assets. The Rust server binds to all interfaces on port 3000, so keep development access restricted to your machine or trusted network.

### Experimental download button

The VOD download endpoint works without this flag. To show the experimental button in the interface, create `front/.env.local` with:

```dotenv
VITE_ENABLE_EXPERIMENTAL=true
```

Then run `npm run build:assets` again. This is a frontend build setting, not a runtime server setting. The published Docker build does not enable it.

## Validation

After installing dependencies and building assets:

```sh
cargo fmt --check
cargo test --locked
node --test front/src/utils/*.test.mjs
npm --prefix front run build
cargo build --release --locked
```

Rust tests cover configuration, audio selection, HLS rewriting, outbound request validation, chat parsing, cache behavior, and streaming process cleanup. The ignored child fixture is launched by its cleanup test. Frontend tests cover chat, favorites, instance settings, and message batching.

When changing dependencies, run `npm audit` at the repository root and in `front`. Do not expose the development server publicly.

To check the complete container build and start it locally:

```sh
docker build -t twinr .
docker run --rm -p 127.0.0.1:3000:3000 twinr
```

The Docker build runs frontend tests/build, Rust formatting checks, release tests, and a release build. Pull-request CI also smoke-tests the container and rejected proxy destinations. Builds published from `main` are available at `ghcr.io/gevroska/twinr:latest`.
