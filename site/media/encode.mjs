import { mkdir, readdir, readFile, writeFile, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { captures, output, run, files, probe } from './process.mjs';

export async function encodeReadme() {
  const destination = fileURLToPath(new URL('../../docs/assets/', import.meta.url));
  const metadata = JSON.parse(await readFile(`${captures}/capture.json`, 'utf8'));
  if (metadata.ai?.completed !== true) throw new Error('The AI capture must be complete before encoding README demos');
  await mkdir(destination, { recursive: true });
  const demos = [];
  for (const [name, stem, width] of [['flow', 'workspace', null], ['debugger', 'debugger', 1280], ['ai', 'ai', 1280]]) {
    const master = `${captures}/${name}.mp4`;
    const info = await probe(master);
    const pixels = width ?? info.width;
    const timing = name === 'ai' && info.duration > 10 ? ['-ss', String(info.duration - 8)] : [];
    const path = `${destination}${stem}-demo.webp`;
    await run('ffmpeg', ['-v', 'error', '-y', ...timing, '-i', master, '-t', '8',
      '-vf', `fps=12,scale=${pixels}:-1:flags=lanczos,format=bgra`, '-c:v', 'libwebp_anim',
      '-lossless', '1', '-compression_level', '6', '-loop', '0', '-an', path]);
    demos.push({ ...((await files([path]))[0]), width: pixels, height: info.height * pixels / info.width,
      fps: 12, duration: Math.min(8, info.duration) });
  }
  return { command: 'media:readme', environment: 'local-media', success: true, files: demos };
}

export async function encode() {
  await mkdir(output, { recursive: true });
  const sources = await readdir(captures);
  const stills = sources.filter(name => name.endsWith('.png')).map(name => name.slice(0, -4)).sort();
  if (!stills.length) throw new Error('No captures; run pnpm ops media:capture first');
  const produced = [];
  for (const name of stills) {
    for (const width of [1600, 800]) {
      const stem = `${output}${name}${width === 800 ? '-800' : ''}`;
      const scale = `scale=${width}:${width * 5 / 8}:flags=lanczos`;
      const common = ['-v', 'error', '-y', '-i', `${captures}/${name}.png`, '-vf', scale, '-frames:v', '1'];
      await run('ffmpeg', [...common, '-c:v', 'libwebp', '-quality', '82', '-compression_level', '6', `${stem}.webp`]);
      await run('ffmpeg', [...common, '-c:v', 'libaom-av1', '-still-picture', '1', '-crf', '32', '-cpu-used', '6', '-pix_fmt', 'yuv420p', '-threads', '4', `${stem}.avif`]);
      produced.push(`${stem}.webp`, `${stem}.avif`);
    }
  }
  const videos = [];
  const metadata = JSON.parse(await readFile(`${captures}/capture.json`, 'utf8'));
  for (const source of sources.filter(name => name.endsWith('.mp4')).sort()) {
    const name = source.slice(0, -4);
    if (name === 'ai' && metadata.ai?.completed !== true) {
      await rm(`${output}ai.webm`, { force: true });
      await rm(`${output}ai.mp4`, { force: true });
      continue;
    }
    if (!stills.includes(name)) throw new Error(`Video ${name} needs a matching poster PNG`);
    const master = `${captures}/${source}`;
    const info = await probe(master);
    const variants = [{ suffix: '', width: 1600, fps: 60 }];
    if (['flow', 'debugger'].includes(name)) variants.push({ suffix: '-120', width: 1280, fps: 120 });
    const bytes = {};
    for (const variant of variants) {
      const { suffix, width, fps } = variant;
      // The AI scene removes most provider waiting; it contains only live footage.
      const timing = name === 'ai' && info.duration > 10 ? ['-ss', String(info.duration - 8), '-t', '8'] : name === 'flow' ? ['-t', '8.5'] : name === 'debugger' ? ['-t', '9.5'] : [];
      const common = ['-v', 'error', '-y', ...timing, '-i', master, '-vf', `scale=${width}:${width * 5 / 8}:flags=lanczos:out_range=tv,fps=${fps}`, '-an', '-pix_fmt', 'yuv420p', '-color_range', 'tv'];
      const webm = `${output}${name}${suffix}.webm`;
      await run('ffmpeg', [...common, '-c:v', 'libsvtav1', '-preset', '6', '-crf', '38', '-svtav1-params', 'lp=4', webm]);
      produced.push(webm);
      bytes[`${name}${suffix}.webm`] = (await files([webm]))[0].bytes;
      if (!suffix) {
        const mp4 = `${output}${name}.mp4`;
        await run('ffmpeg', [...common, '-c:v', 'libx264', '-profile:v', 'high', '-preset', 'slow', '-crf', '25', '-movflags', '+faststart', '-threads', '4', mp4]);
        produced.push(mp4);
        bytes[`${name}.mp4`] = (await files([mp4]))[0].bytes;
      }
    }
    const final = await probe(`${output}${name}.mp4`);
    videos.push({ name, duration: final.duration, fps: variants.map(variant => variant.fps), bytes });
  }
  const manifest = `${output}manifest.json`;
  await writeFile(manifest, `${JSON.stringify({ stills, videos }, null, 2)}\n`);
  produced.push(manifest);
  const assets = await files(produced);
  const inventory = await Promise.all(assets.map(async asset => {
    if (asset.path.endsWith('.json')) return asset;
    const info = await probe(asset.path);
    return /\.(avif|webp)$/.test(asset.path) ? { ...asset, width: info.width, height: info.height } : { ...asset, ...info };
  }));
  const inventoryPath = `${captures}/inventory.json`;
  await writeFile(inventoryPath, `${JSON.stringify({ environment: 'omabox', assets: inventory, webBytes: assets.reduce((sum, asset) => sum + asset.bytes, 0) }, null, 2)}\n`);
  return { command: 'media:encode', environment: 'omabox', success: true, files: [...assets, ...await files([inventoryPath])], videos, warnings: metadata.warnings ?? [] };
}
