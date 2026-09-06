// Animation frames pause in background tabs; retain only the existing UI limit.
export function createMessageBatch(flush, schedule = requestAnimationFrame, cancel = cancelAnimationFrame) {
  let pending = [], frame, disposed = false;
  return {
    push(messages) {
      if (disposed || !messages.length) return;
      pending = pending.concat(messages).slice(-1000);
      if (frame !== undefined) return;
      frame = schedule(() => {
        frame = undefined;
        const batch = pending;
        pending = [];
        if (!disposed) flush(batch);
      });
    },
    dispose() {
      disposed = true;
      if (frame !== undefined) cancel(frame);
      pending = [];
    },
  };
}
