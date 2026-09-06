export function createMessageBatch<T>(flush: (messages: T[]) => void, schedule?: typeof requestAnimationFrame, cancel?: typeof cancelAnimationFrame): { push(messages: T[]): void; dispose(): void };
