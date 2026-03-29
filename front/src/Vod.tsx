import { A, useParams, useSearchParams } from "@solidjs/router";
import {
  Component,
  For,
  Show,
  createEffect,
  createSignal,
  onCleanup,
  lazy,
} from "solid-js";
import axios from "axios";
import Hls from "hls.js";
import dompurify from "dompurify";
import {
  vodCommentsApiResponse,
  vodsApiResponse,
  vodCommentsDataApiResponse,
} from "./utils/types";
import Nav from "./components/nav";
import { BiSolidDownload, BiRegularX } from "solid-icons/bi";
import genericResponseObject from "../../src/types/genericResponseObject";

const DownloadVods = lazy(() => import("./components/downloadVod"));

const Vods: Component = () => {
  const instanceBaseUrl = window.location.origin,
    [{ ...queryParams }, setQueryParams] = useSearchParams(),
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
    streamUrl = `${instanceBaseUrl}/api/vod/${id}${queryString}`,
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

  let hlsInstance: Hls,
    mediaRef: HTMLMediaElement,
    scroll: HTMLDivElement,
    playbackListenerRef: ((ev: Event) => void) | undefined,
    chatRetryTimeout: number | undefined;
  const isAudioOnly = () => String(queryParams.quality || "") === "audio_only";

  if (!Hls.isSupported()) setHlsSuportStatus(false);

  const initHlsStream = () => {
    if (Hls.isSupported()) {
      hlsInstance = new Hls({
        backBufferLength: 9,
        liveSyncDuration: 9,
        manifestLoadingMaxRetry: Infinity,
        manifestLoadingRetryDelay: 500,
      });

      const retry = () => {
        hlsInstance.attachMedia(mediaRef);
        hlsInstance.loadSource(streamUrl);
        hlsInstance.startLoad();
      };

      hlsInstance.attachMedia(mediaRef);

      hlsInstance.on(Hls.Events.MEDIA_ATTACHED, () =>
        hlsInstance.loadSource(streamUrl)
      );
      hlsInstance.on(Hls.Events.MANIFEST_PARSED, () => mediaRef.play());
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

  function sanitizeEvalMessage(content: string) {
    return <span innerHTML={dompurify.sanitize(content)}></span>;
  }

  const fetchComments = async (offset: number) => {
    const req = await axios.get(
        `${instanceBaseUrl}/api/vodinfo/comments/${id}/${offset}`,
        {
          headers: {
            "Content-Type": "application/json",
          },
          validateStatus(status) {
            return true;
          },
        }
      ),
      data: vodCommentsApiResponse = req.data;

    if (data.invalid == true || data.valid == false) {
      return false;
    }
    setVodComments((prev) => [...prev, ...data.data!]);
    return true;
  };
  async function initChat(
    offset: number = 0,
    emoteList: {
      id: string;
      token: string;
      url: string;
    }[]
  ) {
    let commentsStart = 0,
      commentsEnd = 0,
      latestItem: number;
    const fetchCommentsRes = await fetchComments(offset);

    if (fetchCommentsRes == false) {
      // retry after 1s
      chatRetryTimeout = window.setTimeout(
        () => initChat(offset, emoteList),
        1000
      );
      return;
    }

    const comments = vodComments();
    commentsStart = comments[0].offset;
    commentsEnd = comments[comments.length - 1].offset;

    console.log(
      `[Log] Chat info\nInit offset: ${commentsStart}\nEnd offset: ${commentsEnd}`
    );
    console.log(`[Log] Loaded ${emoteList.length} emotes.`);

    function playbackListener() {
      const time = Math.round(mediaRef.currentTime);
      if (latestItem == time || time < commentsStart) return;

      latestItem = time;

      // load more comments
      if (time == commentsEnd || time > commentsEnd) {
        mediaRef.removeEventListener("timeupdate", playbackListener);
        initChat(time > commentsEnd ? time : commentsEnd, emoteList);
        return;
      }
      const selectedComments = comments.filter((x) => x.offset == time);

      if (selectedComments.length < 1) return;

      selectedComments.forEach((message) => {
        if (emoteList.length > 0) {
          const hasEmote = emoteList.some((em) =>
            message.message.includes(em.token)
          );

          if (hasEmote) {
            const emoteByToken = emoteList.filter((em) =>
              message.message?.includes(em.token)
            );

            message.emote = true;
            emoteByToken.map((em) => {
              message.message = message.message?.replace(
                new RegExp(`${em?.token}`, "g"),
                `<img class="inline-flex items-center" src="${em?.url}" alt="${em.token}" height="20" width="20" />`
              );
            });
          }
        }

        const length = comments.length;
        if (length > 1000) {
          setChatMessages([...comments.splice(0, length - 1000), message]);
        } else setChatMessages((prev) => [...prev, message]);
      });

      scroll.scrollTop = scroll.scrollHeight;
    }
    if (playbackListenerRef) {
      mediaRef.removeEventListener("timeupdate", playbackListenerRef);
    }
    if (playbackListenerRef) {
      mediaRef.removeEventListener("timeupdate", playbackListenerRef);
    }
    if (playbackListenerRef) {
      mediaRef.removeEventListener("timeupdate", playbackListenerRef);
    }
    mediaRef.addEventListener("timeupdate", playbackListener);
  };
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

      const emoteListReq = await axios.get(
          `${instanceBaseUrl}/api/emotes/${data.loginName}`
        ),
        emoteListData: genericResponseObject<
          {
            id: string;
            token: string;
            url: string;
          }[]
        > = emoteListReq.data;

      setVodInfo(data);
      setValidStatus(true);
      setReadyStatus(true);
      initChat(0, emoteListData.data || []);
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
    if (hlsInstance) {
      hlsInstance.destroy();
    }

    if (playbackListenerRef && mediaRef) {
      mediaRef.removeEventListener("timeupdate", playbackListenerRef);
    }

    if (chatRetryTimeout) {
      clearTimeout(chatRetryTimeout);
    }
  });

  createEffect(() => {
    if (isReady() == true && isValid() == true) {
      initHlsStream();
    }
  });

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

  const handleResolutionChange = (quality: string) => {
    const nextParams = { ...queryParams };
    if (quality.length < 1) delete nextParams.quality;
    else nextParams.quality = quality;
    setQueryParams(nextParams);
  };

  return (
    <>
      <Nav isHome={false} />
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
          <div class="container mx-auto px-4 py-3 md:py-5">
            <div class="mx-auto flex w-full max-w-7xl flex-col gap-4 lg:grid lg:grid-cols-[minmax(0,2fr)_minmax(320px,1fr)] lg:items-start">
              <div class="w-full">
                <Show
                  when={isAudioOnly()}
                  fallback={
                    <video
                      ref={mediaRef}
                      controls
                      class="w-full rounded-md"
                    />
                  }
                >
                  <audio ref={mediaRef} controls class="w-full" />
                </Show>
                <div class="mt-2">
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
                    <For each={resolutionOptions}>
                      {(option) => (
                        <option value={option.value}>{option.label}</option>
                      )}
                    </For>
                  </select>
                </div>
                <div class="p-1">
                  {isDownloadEnabled == true ? (
                    <Show when={isDownloadSectionOpen() == true}>
                      <div class="mt-1 mb-2">
                        <DownloadVods
                          id={id}
                          queryString={queryString}
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
                  <A
                    class="mt-1 flex flex-row space-x-1"
                    href={`/${vodInfo()?.loginName}${queryString}`}
                  >
                    <img
                      class="w-8 rounded-full"
                      id="avatar"
                      src={`${instanceBaseUrl}/api/proxy?url=${base64encode(
                        vodInfo()?.avatar!
                      )}`}
                    />
                    <span class="ml-1">{vodInfo()?.username}</span>
                  </A>
                </div>
              </div>
              <div class="w-full">
                <div class="border border-base-200 rounded-md shadow-md p-4 w-auto">
                  <h2 class="text-xl">Chat</h2>
                  <div
                    class="mt-3 h-72 md:h-96 lg:h-[70vh] overflow-auto break-words"
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
                          {item.emote === true ? (
                            sanitizeEvalMessage(item.message)
                          ) : (
                            <span>{item.message}</span>
                          )}
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
