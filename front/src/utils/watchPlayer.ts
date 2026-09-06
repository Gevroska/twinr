import { createEffect, onCleanup } from "solid-js";
import Hls from "hls.js";

// Keep the same media element and master playlist when selecting an HLS quality.
export function createWatchPlayer(ready: () => boolean, media: () => HTMLMediaElement, source: string, quality: () => string, reportError: (message: string) => void) {
  let hls: Hls | undefined;
  let activeSource = "";
  let recovered = false;
  const play = () => { void media().play().catch(() => {}); };
  const applyQuality = () => {
    if (!hls?.levels.length) return;
    const selected = quality();
    let level = -1;
    if (selected) {
      level = hls.levels.findIndex(item => selected === "audio_only" ? !item.height : item.height === Number(selected));
      if (level < 0) {
        reportError("This resolution is unavailable for this video. Using Auto.");
      } else reportError("");
    } else reportError("");
    if (hls.currentLevel !== level || (level === -1 && !hls.autoLevelEnabled)) hls.currentLevel = level;
    media().dataset.autoQuality = String(hls.autoLevelEnabled);
  };
  createEffect(() => {
    if (!ready()) return;
    const selected = quality();
    const url = (selected.startsWith("audio_opus_") || selected === "audio_only") ? `${source}?quality=${encodeURIComponent(selected)}` : source;
    if (url === activeSource) { applyQuality(); return; }
    const resume = !activeSource || !media().paused;
    const position = Number.isFinite(media().duration) ? media().currentTime : 0;
    activeSource = url;
    hls?.destroy();
    hls = undefined;
    recovered = false;
    reportError("");
    if (selected.startsWith("audio_opus_")) {
      media().src = url;
      if (resume) play();
      return;
    }
    if (!Hls.isSupported()) {
      media().src = url;
      if (resume) play();
      return;
    }
    hls = new Hls({ backBufferLength: 9, maxBufferLength: 16, maxMaxBufferLength: 32, manifestLoadingMaxRetry: 3, manifestLoadingRetryDelay: 500, startPosition: position || -1 });
    hls.on(Hls.Events.MEDIA_ATTACHED, () => hls?.loadSource(url));
    hls.on(Hls.Events.MANIFEST_PARSED, () => { applyQuality(); if (resume) play(); });
    hls.on(Hls.Events.LEVEL_SWITCHED, (_, data) => {
      media().dataset.qualityHeight = String(hls?.levels[data.level]?.height || 0);
      media().dataset.autoQuality = String(hls?.autoLevelEnabled);
    });
    hls.on(Hls.Events.ERROR, (_, data) => {
      if (!data.fatal) return;
      if (data.type === Hls.ErrorTypes.MEDIA_ERROR && !recovered) {
        recovered = true;
        hls?.recoverMediaError();
      } else {
        hls?.stopLoad();
        reportError("Unable to play this video. Please try again.");
      }
    });
    hls.attachMedia(media());
  });
  onCleanup(() => hls?.destroy());
}
