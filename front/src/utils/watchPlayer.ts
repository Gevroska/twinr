import { createEffect, onCleanup } from "solid-js";
import type Hls from "hls.js";

// Keep the same media element and master playlist when selecting an HLS quality.
export function createWatchPlayer(ready: () => boolean, media: () => HTMLMediaElement, source: string, quality: () => string, reportError: (message: string) => void, reportSupport: (supported: boolean) => void = () => {}) {
  let hls: Hls | undefined;
  let activeSource = "";
  let recovered = false;
  let generation = 0;
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
    const current = ++generation;
    hls?.destroy();
    hls = undefined;
    media().pause();
    media().removeAttribute("src");
    media().load();
    recovered = false;
    reportError("");
    reportSupport(true);
    if (selected.startsWith("audio_opus_")) {
      media().src = url;
      if (resume) play();
      return;
    }
    void import("hls.js").then(({ default: Hls }) => {
      if (current !== generation) return;
      if (!Hls.isSupported()) {
        reportSupport(Boolean(media().canPlayType("application/vnd.apple.mpegurl")));
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
    }).catch(() => {
      if (current === generation) {
        activeSource = "";
        reportError("Unable to load the player. Please refresh and try again.");
      }
    });
  });
  onCleanup(() => {
    ++generation;
    hls?.destroy();
    // Also close native Opus responses, releasing their server-side encoder.
    const element = media();
    if (element) { element.pause(); element.removeAttribute("src"); element.load(); }
  });
}
