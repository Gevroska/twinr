import { A, useParams, useSearchParams } from "@solidjs/router";
import {
  Component,
  For,
  Show,
  createMemo,
  createSignal,
  onCleanup,
  lazy,
} from "solid-js";
import axios from "axios";
import Hls from "hls.js";
import ChatMessage from "./components/chatMessage";
import {
  vodCommentsApiResponse,
  vodsApiResponse,
  vodCommentsDataApiResponse,
} from "./utils/types";
import FavBtn from "./components/favCh";
import Nav from "./components/nav";
import WatchDetails from "./components/watchDetails";
import ChatHeader from "./components/chatHeader";
import { createWatchPlayer } from "./utils/watchPlayer";
import { BiSolidDownload, BiRegularX } from "solid-icons/bi";

const DownloadVods = lazy(() => import("./components/downloadVod"));

const Vods: Component = () => {
  const [mobileNavOpen, setMobileNavOpen] = createSignal(false);
  const instanceBaseUrl = window.location.origin,
    [queryParams, setQueryParams] = useSearchParams(),
    { id } = useParams(),
    [isReady, setReadyStatus] = createSignal(false),
    [isValid, setValidStatus] = createSignal<boolean>(),
    [isHlsSupported, setHlsSuportStatus] = createSignal(true),
    [vodInfo, setVodInfo] = createSignal<vodsApiResponse>(),
    [vodComments, setVodComments] = createSignal<vodCommentsDataApiResponse[]>(
      []
    ),
    [chatMessages, setChatMessages] = createSignal<
      vodCommentsDataApiResponse[]
    >([]),
    [isDownloadSectionOpen, setIsDownloadSectionOpen] = createSignal(false),
    [loadingError, setLoadingError] = createSignal(""),
    [playbackError, setPlaybackError] = createSignal(""),
    [opusAudioBitrates, setOpusAudioBitrates] = createSignal<number[]>([]),
    queryString = createMemo(() => {
      const params = new URLSearchParams();
      Object.entries(queryParams).forEach(([key, value]) => {
        if (value !== undefined && value !== "") params.set(key, String(value));
      });
      const query = params.toString();
      return query ? `?${query}` : "";
    }),
    isDownloadEnabled = import.meta.env.VITE_ENABLE_EXPERIMENTAL === "true",
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

  let mediaRef!: HTMLVideoElement;
  let scroll!: HTMLDivElement;
  let playbackListenerRef: ((ev: Event) => void) | undefined;
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

  let chatDisposed = false;
  let commentRequest = 0;
  let lastChatTime = -1;
  let commentEnd = -1;
  let commentsLoading = false;
  const fetchComments = async (offset: number) => {
    const request = ++commentRequest;
    commentsLoading = true;
    try {
      const response = await axios.get(`${instanceBaseUrl}/api/vodinfo/comments/${id}/${Math.floor(offset)}?format=fragments`);
      if (chatDisposed || request !== commentRequest) return;
      const data = response.data as vodCommentsApiResponse;
      const comments = data.data || [];
      setVodComments(comments);
      commentEnd = comments.length ? comments[comments.length - 1].offset : offset + 30;
    } catch {
      commentEnd = offset + 5;
    } finally {
      if (request === commentRequest) commentsLoading = false;
    }
  };
  function initChat() {
    void fetchComments(0);
    playbackListenerRef = () => {
      const time = mediaRef.currentTime;
      if (Math.abs(time - lastChatTime) > 5 && lastChatTime >= 0) {
        setChatMessages([]);
        void fetchComments(time);
        lastChatTime = time - 1;
        return;
      }
      if (commentsLoading) return;
      const comments = vodComments().filter(item => item.offset > lastChatTime && item.offset <= time);
      if (comments.length) {
        const follow = scroll.scrollHeight - scroll.scrollTop - scroll.clientHeight < 80;
        setChatMessages(previous => [...previous, ...comments].slice(-1000));
        if (follow) scroll.scrollTop = scroll.scrollHeight;
      }
      lastChatTime = time;
      if (time >= commentEnd) void fetchComments(time);
    };
    mediaRef.addEventListener("timeupdate", playbackListenerRef);
  }
  async function fetchVodInfo() {
    try {
      setLoadingError("");
      const req = await axios.get(`${instanceBaseUrl}/api/vodinfo/${id}`, {
          headers: {
            "Content-Type": "application/json",
          },
          validateStatus(status) {
            return true;
          },
        }),
        data: vodsApiResponse & { valid?: boolean } = req.data;

      if (req.status !== 200 || data.invalid == true || data.valid == false) {
        setLoadingError(
          "Unable to load VOD details right now. Please refresh and try again."
        );
        setValidStatus(false);
        setReadyStatus(true);
        return;
      }

      setVodInfo(data);
      setValidStatus(true);
      setReadyStatus(true);
      initChat();
    } catch (err) {
      console.error("[Vod] Failed to load VOD info:", err);
      setLoadingError(
        "Unable to load VOD details right now. Please refresh and try again."
      );
      setValidStatus(false);
      setReadyStatus(true);
    }
  }

  onCleanup(() => {
    chatDisposed = true;

    if (playbackListenerRef && mediaRef) {
      mediaRef.removeEventListener("timeupdate", playbackListenerRef);
    }

  });

  createWatchPlayer(() => isReady() && isValid() === true, () => mediaRef,
    `${instanceBaseUrl}/api/vod/${id}`, () => String(queryParams.quality || ""), setPlaybackError);

  const loadingWatchdog = window.setTimeout(() => {
    if (isReady() == false) {
      setLoadingError("Loading timed out. Please refresh and try again.");
      setValidStatus(false);
      setReadyStatus(true);
    }
  }, 20000);

  onCleanup(() => {
    clearTimeout(loadingWatchdog);
  });

  fetchVodInfo();
  axios
    .get(`${instanceBaseUrl}/api`, {
      headers: {
        "Content-Type": "application/json",
      },
      validateStatus(status) {
        return true;
      },
    })
    .then((res) => {
      const bitrates = Array.isArray(res.data?.opusAudioBitrates)
        ? res.data.opusAudioBitrates
            .map((item: unknown) => Number(item))
            .filter((item: number) => Number.isFinite(item) && item > 0)
        : [];
      setOpusAudioBitrates(bitrates);
    })
    .catch((err) => {
      console.warn("[Vod] Failed to load Opus audio settings:", err);
    });

  const handleResolutionChange = (quality: string) => {
    setQueryParams({ quality: quality || undefined });
  };

  return (
    <>
      <Nav isHome={false} mobileOpen={mobileNavOpen()} />
      <Show when={isHlsSupported() == false}>
        <div class="container mx-auto my-auto px-10 py-2">
          <div class="border p-2 rounded-md shadow-md border-base-200">
            Your browser don't support HLS.
          </div>
        </div>
      </Show>
      <Show when={isReady() == false}>
        <div class="flex justify-center items-center h-screen flex-col">
          <span class="loading loading-spinner text-secondary"></span>
          Loading..
        </div>
      </Show>
      <Show when={isReady() == true}>
        <title>{vodInfo()?.title}</title>
        <Show when={isValid() == false}>
          <div class="container mx-auto my-auto px-10 py-2">
            <div class="border p-2 rounded-md shadow-md border-base-200">
              <Show
                when={loadingError().length > 0}
                fallback={<>Invalid VOD.</>}
              >
                {loadingError()}
              </Show>
            </div>
          </div>
        </Show>
        <Show when={isValid() == true}>
          <div class="watch-page" data-nav-open={mobileNavOpen()}>
            <div class="watch-layout">
              <div class="watch-column">
                <video ref={mediaRef} controls playsinline class="watch-video" classList={{"watch-audio": isAudioOnly() || isOpusAudioOnly()}} />
                <Show when={playbackError()}>
                  <p role="alert" class="mt-2">{playbackError()}</p>
                </Show>
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
                  {isDownloadEnabled == true ? (
                    <Show when={isDownloadSectionOpen() == true}>
                      <div class="mt-1 mb-2">
                        <DownloadVods
                          id={id}
                          queryString={queryString()}
                          streamer={vodInfo()?.username!}
                          title={vodInfo()?.title!}
                        />
                      </div>
                    </Show>
                  ) : (
                    <></>
                  )}
                  <h2 class="text-lg font-semibold">
                    {vodInfo()?.title}{" "}
                    {isDownloadEnabled == true ? (
                      <button
                        class="btn btn-xs"
                        onclick={() =>
                          setIsDownloadSectionOpen(!isDownloadSectionOpen())
                        }
                      >
                        <Show when={isDownloadSectionOpen() == false}>
                          <BiSolidDownload fill="#FFFF" />
                        </Show>
                        <Show when={isDownloadSectionOpen() == true}>
                          <BiRegularX fill="#FFFF" />
                        </Show>
                      </button>
                    ) : (
                      <></>
                    )}
                  </h2>
                  <span class="text-indigo-400">{vodInfo()?.game}</span>
                  <div class="watch-channel-row mt-1 flex items-center gap-2"><A
                    class="flex items-center gap-1"
                    href={`/${vodInfo()?.loginName}${queryString()}`}
                  >
                    <img
                      class="w-8 rounded-full"
                      id="avatar"
                      src={`${instanceBaseUrl}/api/proxy?url=${base64encode(
                        vodInfo()?.avatar!
                      )}`}
                    />
                    <span class="ml-1">{vodInfo()?.username}</span>
                  </A><FavBtn username={vodInfo()?.loginName || ""} /></div>
                </div>
                </WatchDetails>
              </div>
              <div class="watch-chat">
                <div class="watch-chat-panel">
                  <ChatHeader expanded={mobileNavOpen()} toggle={() => setMobileNavOpen(v => !v)} />
                  <div
                    class="watch-chat-messages"
                    style={{
                      "scrollbar-width": "thin",
                    }}
                    ref={scroll}
                  >
                    <For each={chatMessages()}>
                      {(item) => (
                        <div>
                          <span
                            style={{
                              color: item.color,
                            }}
                          >
                            {item.username}
                          </span>
                          :{" "}
                          <ChatMessage fragments={item.fragments} message={item.message} />
                        </div>
                      )}
                    </For>
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

export default Vods;
