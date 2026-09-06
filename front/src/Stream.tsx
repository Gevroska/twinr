import {
  Component,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  lazy,
  onMount,
  onCleanup,
} from "solid-js";
import { useSearchParams, useParams } from "@solidjs/router";
import axios from "axios";
import Hls from "hls.js";
import Nav from "./components/nav";
import WatchDetails from "./components/watchDetails";
import ChatHeader from "./components/chatHeader";
import { createWatchPlayer } from "./utils/watchPlayer";
import FavBtn from "./components/favCh";
import LiveMetadata from "./components/liveMetadata";
import {
  clipsResponse,
  streamStatusResponse,
  streamerMetadataResponse,
  vodsResponse,
} from "./utils/types";
import VodsContainer from "./components/vodsContainer";

const ClipsContainer = lazy(() => import("./components/clipsContainer")),
  StreamChat = lazy(() => import("./components/streamChat"));

const Stream: Component = () => {
  const [mobileNavOpen, setMobileNavOpen] = createSignal(false);
  const instanceBaseUrl = window.location.origin,
    [queryParams, setQueryParams] = useSearchParams(),
    { ...params } = useParams(),
    [isLive, setLiveStatus] = createSignal(false),
    [streamMetadata, setStreamMetadata] = createSignal<streamStatusResponse>(),
    [visibleTab, setVisibleTab] = createSignal(""),
    [visibleTabData, setVisibleTabData] = createSignal<
      vodsResponse | clipsResponse
    >(),
    [streamerMetadata, setStreamerMetadata] =
      createSignal<streamerMetadataResponse>(),
    [videosFilter, setVideosFilter] = createSignal("ARCHIVE"),
    [clipsFilter, setClipsFilter] = createSignal("LAST_DAY"),
    [isHlsSupported, setHlsSuportStatus] = createSignal(true),
    [isReady, setReadyStatus] = createSignal<boolean>(false),
    [isVodlistReady, setVodlistReadyStatus] = createSignal(false),
    [isCliplistReady, setCliplistReadyStatus] = createSignal(false),
    [loadingError, setLoadingError] = createSignal(""),
    [opusAudioBitrates, setOpusAudioBitrates] = createSignal<number[]>([]),
    queryLimit = 100,
    requestConfig = {
      headers: {
        "Content-Type": "application/json",
      },
      timeout: 15000,
      validateStatus(status: number) {
        return true;
      },
    },
    queryString = createMemo(() => {
      const query = new URLSearchParams();
      Object.entries(queryParams).forEach(([key, value]) => { if (value) query.set(key, String(value)); });
      return query.size ? `?${query}` : "";
    }),
    base64encode = (content: string) => btoa(content);
  const resolutionOptions = [
    { value: "", label: "Auto" },
    { value: "1080", label: "1920x1080" },
    { value: "720", label: "1280x720" },
    { value: "480", label: "852x480" },
    { value: "360", label: "640x360" },
    { value: "160", label: "284x160" },
    { value: "audio_only", label: "Audio only" },
  ];
  const safeUsername = String(params.username || "").toLowerCase();
  let mediaRef!: HTMLVideoElement;
  let chatScroll!: HTMLDivElement;
  const isAudioOnly = () => String(queryParams.quality || "") === "audio_only";
  const isOpusAudioOnly = () =>
    String(queryParams.quality || "").startsWith("audio_opus_");
  const allResolutionOptions = () => [
    ...resolutionOptions,
    ...opusAudioBitrates().map((bitrate) => ({
      value: `audio_opus_${bitrate}`,
      label: `Audio only (Opus ${bitrate} kbps)`,
    })),
  ];

  if (!Hls.isSupported()) setHlsSuportStatus(false);

  const fetchStreamerInfo = async (retryCount: number = 0) => {
    try {
      setLoadingError("");
      const req = await axios.get(
          `${instanceBaseUrl}/api/streaminfo/${params.username}`,
          requestConfig
        ),
        data = req.data as streamStatusResponse & { valid?: boolean };

      if (req.status >= 500) throw new Error("Channel metadata is temporarily unavailable");

      if (req.status !== 200 || data.invalid == true || data.valid == false) {
        setLiveStatus(false);
        const streamerMetadataReq = await axios.get(
            `${instanceBaseUrl}/api/streamer/${params.username}`,
            requestConfig
          ),
          streamerMetadataRes =
            streamerMetadataReq.data as streamerMetadataResponse;

        if (streamerMetadataRes.invalid !== true) {
          setStreamerMetadata(streamerMetadataRes);
          setVisibleTab("videos");
          setVodlistReadyStatus(true);
          setReadyStatus(true);
        } else {
          setReadyStatus(true);
          setLoadingError(
            "Unable to load streamer information right now. Please retry."
          );
        }
      } else {
        setStreamMetadata(data);
        setLiveStatus(true);
        setReadyStatus(true);
      }
    } catch (err) {
      const isCanceledError =
        axios.isAxiosError(err) && err.code === "ERR_CANCELED";

      if (retryCount < 3) {
        const delay = 500 * (retryCount + 1);
        console.warn(
          `[Stream] Failed to fetch streamer info${
            isCanceledError ? " (request aborted)" : ""
          }. Retrying in ${delay}ms.`,
          err
        );
        window.setTimeout(() => fetchStreamerInfo(retryCount + 1), delay);
        return;
      }

      console.error("[Stream] Failed to fetch streamer info:", err);
      setLoadingError(
        "Unable to load stream details. Please refresh and try again."
      );
      setReadyStatus(true);
    }
  };

  const loadingWatchdog = window.setTimeout(() => {
    if (isReady() == false) {
      setLoadingError("Loading timed out. Please refresh and try again.");
      setReadyStatus(true);
    }
  }, 20000);

  onMount(() => {
    if (safeUsername.length < 1) {
      setLoadingError("Invalid stream URL.");
      setReadyStatus(true);
      return;
    }

    fetchStreamerInfo();
    axios
      .get(`${instanceBaseUrl}/api`, requestConfig)
      .then((res) => {
        const bitrates = Array.isArray(res.data?.opusAudioBitrates)
          ? res.data.opusAudioBitrates
              .map((item: unknown) => Number(item))
              .filter((item: number) => Number.isFinite(item) && item > 0)
          : [];
        setOpusAudioBitrates(bitrates);
      })
      .catch((err) => {
        console.warn("[Stream] Failed to load Opus audio settings:", err);
      });
  });

  const handleResolutionChange = (quality: string) => {
    setQueryParams({ quality: quality || undefined });
  };

  // updating metadata every 1 minute
  const streamMetadataUpdater = setInterval(async () => {
    if (isLive() == true) {
      try {
        console.log("[Log] Updating stream metadata.");
        const req = await axios.get(
            `${instanceBaseUrl}/api/streaminfo/${params.username}`,
            requestConfig
          ),
          data = req.data as streamStatusResponse;

        if (data.invalid !== true) {
          setStreamMetadata(data);
        }
      } catch (err) {
        console.error("[Stream] Failed to update stream metadata:", err);
      }
    }
  }, 60000);

  // tabs handler
  createEffect(async () => {
    try {
      if (visibleTab() == "videos") {
        const req = await axios.get(
            `${instanceBaseUrl}/api/vods/${
              params.username
            }/${videosFilter()}/${queryLimit}`
          ),
          data = req.data as vodsResponse;

        if (data.invalid !== true) {
          setVisibleTabData(data);
        }
      }
      if (visibleTab() == "clips") {
        const req = await axios.get(
            `${instanceBaseUrl}/api/clips/${
              params.username
            }/${clipsFilter()}/${queryLimit}`
          ),
          data = req.data as clipsResponse;

        if (data.invalid !== true) {
          setVisibleTabData(data);
          setCliplistReadyStatus(true);
        }
      }
    } catch (err) {
      console.error("[Stream] Failed to fetch tab data:", err);
    }
  });

  onCleanup(() => {
    clearInterval(streamMetadataUpdater);
    clearTimeout(loadingWatchdog);
  });

  createWatchPlayer(() => isReady() && isLive(), () => mediaRef,
    `${instanceBaseUrl}/api/stream/${params.username}`, () => String(queryParams.quality || ""), setLoadingError);

  return (
    <>
      <Nav isHome={false} mobileOpen={!isLive() || mobileNavOpen()} />
      <Show when={isReady() == false}>
        <div class="flex justify-center items-center h-screen flex-col">
          <span class="loading loading-spinner text-secondary"></span>
          Loading..
          <Show when={loadingError().length > 0}>
            <span class="text-error mt-2 text-center">{loadingError()}</span>
          </Show>
        </div>
      </Show>
      <Show when={isReady() == true}>
        <title>{params.username}</title>
        <Show when={loadingError().length > 0}>
          <div class="container mx-auto my-auto px-10 py-2">
            <div class="border p-2 rounded-md shadow-md border-error text-error">
              {loadingError()}
            </div>
          </div>
        </Show>
        <Show when={isLive() == false || isHlsSupported() == false}>
          <Show when={isHlsSupported() == false}>
            <div class="container mx-auto my-auto px-10 py-2">
              <div class="border p-2 rounded-md shadow-md border-base-200">
                Your browser don't support HLS.
              </div>
            </div>
          </Show>
          <div class="container mx-auto my-auto p-10">
            <div>
              <div
                class="bg-cover bg-center h-40"
                style={{
                  "background-image": `url('${instanceBaseUrl}/api/proxy?url=${base64encode(
                    streamerMetadata()?.bannerImageURL!
                  )}')`,
                }}
              >
                <div class="container mx-auto flex flex-col space-y-2 items-center justify-center h-full">
                  <img
                    class="rounded-full overflow-hidden h-24 w-24"
                    src={`${instanceBaseUrl}/api/proxy?url=${base64encode(
                      streamerMetadata()?.profileImageURL!
                    )}`}
                  />
                  <FavBtn username={params.username} />
                </div>
              </div>
            </div>
            <div class="container mx-auto mt-2 mb-2 bg-neutral-focus p-2 rounded-md shadow-sm">
              <div class="flex items-center justify-center">
                {streamerMetadata()?.description}
              </div>
              <div class="flex flex-col md:flex-row items-center justify-center md:space-x-2 space-x-1">
                <For each={streamerMetadata()?.socialMedias}>
                  {(item) => (
                    <a
                      class="no-underline text-secondary capitalize item"
                      href={item.url}
                    >
                      {item.title}
                    </a>
                  )}
                </For>
              </div>
            </div>
            <div>
              <div class="tabs mt-2">
                <a
                  class={`tab ${visibleTab() == "videos" ? "tab-active" : ""}`}
                  onclick={() => setVisibleTab("videos")}
                >
                  Videos
                </a>
                <a
                  class={`tab ${visibleTab() == "clips" ? "tab-active" : ""}`}
                  onclick={() => setVisibleTab("clips")}
                >
                  Clips
                </a>
              </div>
              <div class="mt-2">
                <Show when={visibleTab() == "videos"}>
                  <VodsContainer
                    setFilter={setVideosFilter}
                    tabData={visibleTabData}
                    queryString={queryString()}
                    instanceBaseUrl={instanceBaseUrl}
                    ready={isVodlistReady}
                  />
                </Show>
                <Show when={visibleTab() == "clips"}>
                  <ClipsContainer
                    setFilter={setClipsFilter}
                    streamer={safeUsername}
                    tabData={visibleTabData}
                    instanceBaseUrl={instanceBaseUrl}
                    ready={isCliplistReady}
                  />
                </Show>
              </div>
            </div>
          </div>
        </Show>
        <Show when={isLive() == true}>
          <div class="watch-page" data-nav-open={mobileNavOpen()}>
            <div class="watch-layout">
              <div class="watch-column">
                <video ref={mediaRef} controls playsinline class="watch-video" classList={{"watch-audio": isAudioOnly() || isOpusAudioOnly()}} />
                <div class="watch-quality">
                  <label class="label p-0">
                    <span class="label-text text-sm">Resolution</span>
                  </label>
                  <select
                    class="select select-bordered select-sm w-full max-w-[220px]"
                    value={String(queryParams.quality || "")}
                    onchange={(e) =>
                      handleResolutionChange(e.currentTarget.value)
                    }
                  >
                    <For each={allResolutionOptions()}>
                      {(option) => (
                        <option value={option.value}>{option.label}</option>
                      )}
                    </For>
                  </select>
                </div>
                <WatchDetails>
                <div class="watch-metadata">
                  <LiveMetadata
                    title={streamMetadata()?.title!}
                    views={streamMetadata()?.views!}
                    game={streamMetadata()?.game!}
                    avatar={`${instanceBaseUrl}/api/proxy?url=${base64encode(
                      streamMetadata()?.avatar!
                    )}`}
                    username={params.username}
                  />
                </div>
                </WatchDetails>
              </div>
              <div class="watch-chat">
                <div class="watch-chat-panel">
                  <ChatHeader expanded={mobileNavOpen()} toggle={() => setMobileNavOpen(v => !v)} />
                  <div class="watch-chat-body">
                    <div
                      class="watch-chat-messages"
                      style={{
                        "scrollbar-width": "thin",
                      }}
                      ref={chatScroll}
                    >
                      <StreamChat username={safeUsername} scroll={chatScroll} />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </Show>
    </>
  );
};

export default Stream;
