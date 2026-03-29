import {
  Component,
  For,
  Show,
  createEffect,
  createSignal,
  lazy,
  onMount,
  onCleanup,
} from "solid-js";
import { useSearchParams, useParams } from "@solidjs/router";
import axios from "axios";
import Hls from "hls.js";
import Nav from "./components/nav";
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
  const instanceBaseUrl = window.location.origin,
    [{ ...queryParams }, setQueryParams] = useSearchParams(),
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
    queryEntries = Object.keys(queryParams).filter(
      (key) =>
        typeof queryParams[key] !== "undefined" && queryParams[key] !== ""
    ),
    queryString =
      queryEntries.length > 0
        ? `?${queryEntries
            .map((key) => {
              return `${key}=${queryParams[key]}`;
            })
            .join("&")}`
        : "",
    streamUrl = `${instanceBaseUrl}/api/stream/${params.username}${queryString}`,
    base64encode = (content: string) => btoa(content),
    encodeProxyUrl = (content: string) =>
      encodeURIComponent(base64encode(content));
  const safeUsername = String(params.username || "").toLowerCase();
  let hlsInstance: Hls, videoRef: HTMLVideoElement, chatScroll: HTMLDivElement;

  if (!Hls.isSupported()) setHlsSuportStatus(false);

  const initHlsStream = () => {
    if (Hls.isSupported()) {
      hlsInstance = new Hls({
        maxBufferLength: 16,
        maxBufferSize: 64 * 1024 * 1024,
        maxMaxBufferLength: 32,
        backBufferLength: 2,
        liveSyncDuration: 2,
        manifestLoadingMaxRetry: Infinity,
        manifestLoadingRetryDelay: 500,
        xhrSetup: (xhr, url) => {
          const normalizedRequestUrl = url.replace(/\?$/, ""),
            normalizedStreamUrl = streamUrl.replace(/\?$/, "");

          if (normalizedRequestUrl !== normalizedStreamUrl) {
            xhr.open(
              "GET",
              `${instanceBaseUrl}/api/proxy?url=${encodeProxyUrl(url)}`
            );
          } else xhr.open("GET", url);
        },
      });

      const retry = () => {
        hlsInstance.attachMedia(videoRef);
        hlsInstance.loadSource(streamUrl);
        hlsInstance.startLoad();
      };

      hlsInstance.attachMedia(videoRef);

      hlsInstance.on(Hls.Events.MEDIA_ATTACHED, () =>
        hlsInstance.loadSource(streamUrl)
      );
      hlsInstance.on(Hls.Events.MANIFEST_PARSED, () => videoRef.play());
      hlsInstance.on(Hls.Events.ERROR, function (_, data) {
        if (data.fatal) {
          switch (data.type) {
            case Hls.ErrorTypes.NETWORK_ERROR:
              console.log("Network error. Retrying..");
              retry();
              break;
            case Hls.ErrorTypes.MEDIA_ERROR:
              console.log("Media error. Retrying..");
              hlsInstance.recoverMediaError();
              break;
          }
        }
      });
    }
  };

  const fetchStreamerInfo = async (retryCount: number = 0) => {
    try {
      setLoadingError("");
      const req = await axios.get(
          `${instanceBaseUrl}/api/streaminfo/${params.username}`,
          requestConfig
        ),
        data = req.data as streamStatusResponse & { valid?: boolean };

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
  });

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
    if (hlsInstance) {
      hlsInstance.destroy();
    }
  });

  // handle hls stream
  createEffect(() => {
    if (isReady() == true && isLive() == true) {
      initHlsStream();
    }
  });

  return (
    <>
      <Nav isHome={false} />
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
                    queryString={queryString}
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
          <div class="container md:py-5">
            <div class="flex justify-center items-center">
              <div class="md:ml-40 flex flex-col md:flex-row md:gap-2">
                <div class="w-full md:w-3/4 md:h-auto">
                  <video ref={videoRef} controls />
                  <div class="p-1">
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
                </div>
                <div class="w-full md:w-2/4">
                  <div class="border border-base-200 rounded-md shadow-md p-4 w-auto">
                    <h2 class="text-xl">Chat</h2>
                    <div
                      class="mt-3 h-96 md:h-80 overflow-auto break-words"
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
