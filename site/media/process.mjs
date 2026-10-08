import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const execute = promisify(execFile);
export const mediaDir = fileURLToPath(new URL('.', import.meta.url));
export const captures = `${mediaDir}captures`;
export const output = fileURLToPath(new URL('../public/media/', import.meta.url));
export async function run(command, args, options = {}) {
  try {
    return await execute(command, args, { maxBuffer: 32 * 1024 * 1024, ...options });
  } catch (error) {
    throw new Error(`${command} failed: ${error.stderr || error.message}`);
  }
}
export const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
export async function files(paths) {
  return Promise.all(paths.map(async path => ({ path, bytes: (await stat(path)).size })));
}
export async function probe(path) {
  const { stdout } = await run('ffprobe', ['-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=width,height,avg_frame_rate,nb_frames:format=duration', '-of', 'json', path]);
  const data = JSON.parse(stdout);
  const stream = data.streams[0];
  const [num, den] = stream.avg_frame_rate.split('/').map(Number);
  return { width: stream.width, height: stream.height, fps: num / den, duration: Number(data.format.duration) };
}
export async function cadence(path) {
  // Default mpdecimate discards subtle UI motion; keep small pointer/seek-bar changes.
  const { stderr } = await run('ffmpeg', ['-hide_banner', '-i', path, '-vf', 'mpdecimate=hi=64:lo=32:frac=0.001,showinfo', '-an', '-f', 'null', '-']);
  const times = [...stderr.matchAll(/pts_time:([\d.]+)/g)].map(match => Number(match[1]));
  const deltas = times.slice(1).map((time, i) => (time - times[i]) * 1000);
  const near120 = deltas.filter(delta => delta > 7 && delta < 10);
  return { filter: 'mpdecimate=hi=64:lo=32:frac=0.001', uniqueFrames: times.length, intervalsNear120Hz: near120.length, minimumUniqueIntervalMs: deltas.length ? Math.min(...deltas) : null, intervalNear120HzMedianMs: near120.length ? near120.sort((a, b) => a - b)[Math.floor(near120.length / 2)] : null };
}
