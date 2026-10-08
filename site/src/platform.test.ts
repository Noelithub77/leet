import test from 'node:test';
import assert from 'node:assert/strict';
import { detectPlatform } from './platform';

test('detects desktop platforms without offering a desktop download to mobile devices', () => {
  assert.equal(detectPlatform('Win32', 'Windows NT 10.0'), 'windows');
  assert.equal(detectPlatform('MacIntel', 'Mac OS X'), 'macos');
  assert.equal(detectPlatform('Linux aarch64', 'Linux'), 'linux');
  assert.equal(detectPlatform('Linux armv8', 'Android'), null);
  assert.equal(detectPlatform('iPhone', 'Mac OS X'), null);
  assert.equal(detectPlatform('', ''), null);
});
