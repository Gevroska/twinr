// A tab-local cache: no polling, storage writes, or additional server work.
export function createSettingsLoader(fetcher = fetch, now = Date.now) {
  let pending;
  let expires = 0;
  return () => {
    if (pending && now() < expires) return pending;
    expires = Infinity;
    pending = fetcher('/api', { signal: AbortSignal.timeout(15000) })
      .then(response => {
        if (!response.ok) throw new Error('Instance settings unavailable');
        return response.json();
      })
      .then(data => {
        expires = now() + 60000;
        return Array.isArray(data?.opusAudioBitrates)
          ? [...new Set(data.opusAudioBitrates.map(Number).filter(n => Number.isFinite(n) && n > 0))]
          : [];
      })
      .catch(error => { pending = undefined; expires = 0; throw error; });
    return pending;
  };
}
export const loadInstanceSettings = createSettingsLoader();
