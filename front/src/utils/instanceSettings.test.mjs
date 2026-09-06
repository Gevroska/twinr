import test from 'node:test';
import assert from 'node:assert/strict';
import { createSettingsLoader } from './instanceSettings.mjs';

test('settings share concurrent requests and cache navigation without polling', async () => {
  let calls = 0, time = 0;
  const load = createSettingsLoader(async () => {
    calls++;
    return { ok: true, json: async () => ({ opusAudioBitrates: [32, '64', 64, -1, 'bad'] }) };
  }, () => time);
  const a = load(), b = load();
  assert.equal(a, b);
  assert.deepEqual(await a, [32, 64]);
  await load();
  assert.equal(calls, 1);
  time = 60001;
  assert.equal(calls, 1);
  await load();
  assert.equal(calls, 2);
});
test('settings failures can retry on the next explicit request', async () => {
  let calls = 0;
  const load = createSettingsLoader(async () => ({ ok: ++calls > 1, json: async () => ({}) }));
  await assert.rejects(load());
  assert.deepEqual(await load(), []);
  assert.equal(calls, 2);
});
