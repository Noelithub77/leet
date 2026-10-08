import test from 'node:test';
import assert from 'node:assert/strict';
import { digits, logo, strokeAt } from './logo';
import { lineHeat, maxAreaTrace, sample } from './trace';
import { snapRefreshRate } from './refresh';

test('morph starts at the logo and lands exactly on 1337', () => {
  assert.deepEqual(logo.map((_, index) => strokeAt(index, 0).points), logo.map(stroke => stroke.points));
  assert.deepEqual(logo.map((_, index) => strokeAt(index, 1).points), digits.map(stroke => stroke.points));
  for (const stroke of [...logo, ...digits]) assert.equal(stroke.points.length, 5);
});

test('debugger replay matches leet’s recorded Container With Most Water trace', () => {
  const steps = maxAreaTrace(sample);
  assert.equal(steps.length, 46);
  assert.deepEqual(steps[12], { line: 11, event: 'line', l: 1, r: 8, best: 49, area: 49 });
  assert.equal(steps.at(-1).best, 49);
  assert.deepEqual(maxAreaTrace(sample), steps, 'replays are deterministic');
  assert.equal(lineHeat(steps, steps.length - 1)[5], 9);
});

test('refresh rate snaps frame intervals to common displays', () => {
  assert.equal(snapRefreshRate(new Array(20).fill(8.33)), 120);
  assert.equal(snapRefreshRate(new Array(20).fill(16.7)), 60);
  assert.equal(snapRefreshRate([...new Array(20).fill(6.94), 40, 33]), 144);
  assert.equal(snapRefreshRate([16, 16]), null);
});
