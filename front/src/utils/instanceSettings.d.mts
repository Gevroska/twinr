export function createSettingsLoader(fetcher?: typeof fetch, now?: () => number): () => Promise<number[]>;
export const loadInstanceSettings: () => Promise<number[]>;
