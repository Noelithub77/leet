import test from 'node:test';
import assert from 'node:assert/strict';
import { createHeroCycle } from './hero-motion';

test('hero shape and spelling stay paired through repeated cycles and pauses', () => {
  const glyphs = () => Array.from({ length: 7 }, () => ({ yPercent: 0, rotationX: 0, opacity: 1, filter: 'blur(0px)' }));
  const letters = glyphs();
  const numbers = glyphs();
  const morph = { value: 1 };
  const cycle = createHeroCycle(morph, letters, numbers);
  const assertPair = digits => {
    assert.equal(morph.value, digits ? 1 : 0);
    assert.ok(numbers.every(char => char.opacity === (digits ? 1 : 0)));
    assert.ok(letters.every(char => char.opacity === (digits ? 0 : 1)));
  };
  try {
    assertPair(true);
    for (const round of [0, 1, 10, 100]) {
      cycle.totalTime(round * cycle.duration() + 3);
      assertPair(false);
      cycle.pause();
      assert.equal(cycle.paused(), true);
      cycle.totalTime(round * cycle.duration() + 5.5);
      assertPair(true);
    }
    cycle.totalTime(0);
    assertPair(true);
  } finally {
    cycle.kill();
  }
});
