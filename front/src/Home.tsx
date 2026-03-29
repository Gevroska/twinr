import { Component, createSignal } from "solid-js";
import Nav from "./components/nav";
import { useNavigate } from "@solidjs/router";

const clipRegex = /(.+)?twitch\.tv\/\w+\/clip\/[\w-]+/,
  streamRegex = /(.+)?twitch\.tv\/(.+)/,
  vodRegex = /(.+)?twitch\.tv\/videos\/(\d+)/;

const Home: Component = () => {
  const [inputVal, setInputVal] = createSignal(""),
    [selectedRes, setRes] = createSignal(""),
    redirect = useNavigate();

  function resolveTargetPathFromUrl(rawInput: string): string | null {
    const formattedInput = rawInput.startsWith("http")
      ? rawInput
      : `https://${rawInput}`;

    let targetUrl: URL;
    try {
      targetUrl = new URL(formattedInput);
    } catch {
      return null;
    }

    const normalizedPathname = targetUrl.pathname.replace(/\/+$/, "");

    const vodMatch = normalizedPathname.match(/^\/videos\/(\d+)$/i);
    if (vodMatch) {
      return `/videos/${vodMatch[1]}`;
    }

    const clipMatch = normalizedPathname.match(/^\/([^/]+)\/clip\/([\w-]+)$/i);
    if (clipMatch) {
      return `/${clipMatch[1]}/clip/${clipMatch[2]}`;
    }

    const streamerMatch = normalizedPathname.match(/^\/([^/]+)$/);
    if (streamerMatch) {
      return `/${streamerMatch[1]}`;
    }

    return null;
  }

  function handleSearch() {
    if (inputVal().length < 1) return;
    const rawInput = inputVal().trim();
    const queryArgs: { [key: string]: string | boolean } = {};

    if (selectedRes().length > 1) queryArgs["quality"] = selectedRes();

    const queryParams = new URLSearchParams();
    Object.keys(queryArgs).forEach((key) => {
      queryParams.set(key, String(queryArgs[key]));
    });
    const resultQuery = queryParams.toString();
    const querySuffix = resultQuery.length > 0 ? `?${resultQuery}` : "";

    if (
      rawInput.match(clipRegex) ||
      rawInput.match(streamRegex) ||
      rawInput.match(vodRegex)
    ) {
      const targetPath = resolveTargetPathFromUrl(rawInput);

      if (targetPath) {
        redirect(`${targetPath}${querySuffix}`, {
          replace: false,
          scroll: true,
        });
      }
      return;
    }

    const targetPathFromAnyUrl = resolveTargetPathFromUrl(rawInput);
    if (targetPathFromAnyUrl) {
      redirect(`${targetPathFromAnyUrl}${querySuffix}`, {
        replace: false,
        scroll: true,
      });
      return;
    }
    if (!Number.isNaN(Number(rawInput))) {
      redirect(`/videos/${rawInput}${querySuffix}`, {
        replace: false,
        scroll: true,
      });
      return;
    }

    redirect(`/${rawInput.replace(/^\/+/, "")}${querySuffix}`, {
      replace: false,
      scroll: true,
    });
  }

  return (
    <>
      <Nav isHome={false} />
      <title>Twinr - Home</title>
      <div class="hero min-h-[80vh] px-4">
        <div class="hero-content w-full max-w-xl text-center">
          <div class="card w-full border border-base-200 bg-base-100/90 shadow-xl backdrop-blur">
            <div class="card-body gap-4">
              <h1 class="text-3xl font-bold">Twinr</h1>
              <p class="text-sm text-base-content/70">
                Lightweight Twitch viewer focused on privacy.
              </p>
              <div class="form-control w-full">
                <label class="label">
                  <span class="label-text text-base font-medium">
                    Search stream, VOD, or clip
                  </span>
                </label>
                <input
                  type="text"
                  placeholder="URL, channel name, VOD ID, clip URL..."
                  class="input input-bordered w-full"
                  value={inputVal()}
                  onInput={(e) => setInputVal(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      handleSearch();
                    }
                  }}
                />
                <details class="mt-2 rounded-md border border-base-200 px-4 py-2 text-left">
                  <summary class="cursor-pointer font-medium">Advanced</summary>
                  <select
                    onchange={(e) => setRes(e.target.value)}
                    class="select select-bordered mt-2 w-full"
                  >
                    <option disabled selected>
                      Resolution
                    </option>
                    <option value="1080">1920x1080</option>
                    <option value="720">1280x720</option>
                    <option value="480">852x480</option>
                    <option value="360">640x360</option>
                    <option value="160">284x160</option>
                    <option value="audio">Audio only</option>
                  </select>
                </details>

                <button class="btn btn-secondary mt-3" onClick={handleSearch}>
                  Search
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
};

export default Home;
