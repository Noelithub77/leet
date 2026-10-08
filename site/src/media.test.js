import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';

const page = new URL('../index.html', import.meta.url);
const media = new URL('../public/media/', import.meta.url);

test('all landing posters and lazy video variants have deployable assets', async () => {
  const html = await readFile(page, 'utf8');
  const referenced = new Set([...html.matchAll(/%BASE_URL%media\/([\w.-]+)/g)].map(match => match[1]));
  for (const match of html.matchAll(/data-media="([\w-]+)"\s+data-video(\s+data-hfr)?/g)) {
    referenced.add(`${match[1]}.webm`);
    referenced.add(`${match[1]}.mp4`);
    if (match[2]) referenced.add(`${match[1]}-120.webm`);
  }
  assert.ok(referenced.size > 0);
  for (const name of referenced) {
    const asset = await stat(new URL(name, media));
    assert.ok(asset.isFile() && asset.size > 0, `missing or empty landing media: ${name}`);
  }
  const manifest = JSON.parse(await readFile(new URL('manifest.json', media), 'utf8'));
  for (const video of manifest.videos) {
    for (const [name, bytes] of Object.entries(video.bytes)) {
      assert.equal((await stat(new URL(name, media))).size, bytes, `stale encoded manifest: ${name}`);
    }
  }
});
