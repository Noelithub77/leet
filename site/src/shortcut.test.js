import test from 'node:test';
import assert from 'node:assert/strict';
import { shouldCopyInstallCommand } from './shortcut.js';

const shortcut = { key: 'c', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, repeat: false, editable: false, selection: '', platform: 'linux' };

test('copies the Unix command with Ctrl+C or Cmd+C', () => {
  assert.equal(shouldCopyInstallCommand(shortcut), true);
  assert.equal(shouldCopyInstallCommand({ ...shortcut, ctrlKey: false, metaKey: true, platform: 'macos' }), true);
  assert.equal(shouldCopyInstallCommand({ ...shortcut, key: 'C' }), true);
});

test('preserves selected text, form controls, developer shortcuts, and Windows behavior', () => {
  for (const override of [
    { selection: 'text to copy' }, { editable: true }, { shiftKey: true },
    { altKey: true }, { repeat: true }, { platform: 'windows' },
    { ctrlKey: false }, { key: 'v' },
  ]) assert.equal(shouldCopyInstallCommand({ ...shortcut, ...override }), false);
});
