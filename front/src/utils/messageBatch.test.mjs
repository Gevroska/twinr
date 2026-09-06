import test from 'node:test';
import assert from 'node:assert/strict';
import { createMessageBatch } from './messageBatch.mjs';

test('chat bursts render once per frame, preserving order and bounding background backlog', () => {
  let callback, frames = 0;
  const batches = [];
  const queue = createMessageBatch(items => batches.push(items), cb => { callback = cb; return ++frames; }, () => {});
  queue.push([1, 2]); queue.push([3]);
  assert.equal(frames, 1);
  callback();
  assert.deepEqual(batches, [[1, 2, 3]]);
  for (let i = 0; i < 2000; i++) queue.push([i]);
  assert.equal(frames, 2);
  callback();
  assert.equal(batches[1].length, 1000);
  assert.equal(batches[1][0], 1000);
  assert.equal(batches[1][999], 1999);
});
test('leaving chat cancels pending rendering and ignores late arrivals', () => {
  let callback, canceled;
  const queue = createMessageBatch(() => assert.fail('disposed chat rendered'), cb => { callback = cb; return 7; }, id => { canceled = id; });
  queue.push(['message']); queue.dispose(); callback(); queue.push(['late']);
  assert.equal(canceled, 7);
});
