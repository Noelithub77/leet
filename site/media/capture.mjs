import { mkdir, mkdtemp, rm, cp, copyFile, chmod, realpath, writeFile, readFile } from 'node:fs/promises';
import { resolve, sep } from 'node:path';
import { homedir } from 'node:os';
import { captures, mediaDir, run, sleep, files, probe, cadence } from './process.mjs';

export const sceneNames = ['flow', 'debugger', 'ai', 'home', 'roadmap', 'settings', 'palette', 'problem'];
export const filmSceneNames = ['film-search', 'film-panes', 'film-debug'];
const twoSum = `class Solution:
    def twoSum(self, nums: list[int], target: int) -> list[int]:
        seen = {}
        for i, n in enumerate(nums):
            if target - n in seen:
                return [seen[target - n], i]
            seen[n] = i
`;
const water = `class Solution:
    def maxArea(self, height: list[int]) -> int:
        l, r = 0, len(height) - 1
        best = 0
        while l < r:
            window = [l, r]
            area = (r - l) * min(height[l], height[r])
            best = max(best, area)
            if height[l] < height[r]:
                l += 1
            else:
                r -= 1
        return best
`;

export async function capture(scene, { codexHome, masterDir } = {}) {
  const availableScenes = masterDir ? [...sceneNames, ...filmSceneNames] : sceneNames;
  if (scene && !availableScenes.includes(scene)) throw new Error(`Unknown scene ${scene}; choose ${availableScenes.join(', ')}. Film scenes require --master-dir.`);
  const destination = masterDir ? resolve(masterDir) : captures;
  await mkdir(destination, { recursive: true });
  if (masterDir) {
    const canonical = await realpath(destination);
    for (const protectedDir of [captures, resolve(mediaDir, '../public/media')]) {
      const protectedPath = await realpath(protectedDir);
      if (canonical === protectedPath || canonical.startsWith(`${protectedPath}${sep}`)) throw new Error('Master directory must be outside published capture and web asset directories');
    }
  }
  const scratch = masterDir ? await mkdtemp(`${destination}/.capture-`) : `${mediaDir}.scratch`;
  await mkdir(scratch, { recursive: true });
  const fps = masterDir ? 60 : 120;
  const recorderSettings = masterDir
    ? ['-r', '60', '-c', 'libx264rgb', '-x', 'bgr0', '-p', 'preset=ultrafast', '-p', 'crf=0', '-p', 'threads=4']
    : ['-r', '120', '-c', 'h264_vaapi', '-p', 'qp=16', '-d', '/dev/dri/renderD129'];
  const box = `leet-media-${process.pid}`;
  const boxRun = (...args) => run('omabox', ['run', '-b', box, '--', ...args]);
  const keys = (...args) => run('omabox', ['keys', '-b', box, '--window', 'leet', ...args]);
  const selected = name => !scene || scene === name;
  const filmSelected = name => masterDir && selected(name);
  const produced = [];
  const recordings = [];
  const warnings = [];
  let home;
  let recorder;
  let ai;
  let recordingStarted;
  let boxStarted = false;
  const interrupt = async () => {
    await run('omabox', ['down', box]).catch(() => {});
    process.exit(130);
  };
  process.once('SIGINT', interrupt);
  process.once('SIGTERM', interrupt);
  const shot = async name => {
    await sleep(300);
    const path = `${destination}/${name}.png`;
    await run('omabox', ['shot', '-b', box, '--window', 'leet', '-o', path]);
    // Recompress losslessly; PNG compression_level ranges from 0 to 9 in FFmpeg.
    const temp = `${scratch}/${name}.png`;
    await run('ffmpeg', ['-v', 'error', '-y', '-i', path, '-compression_level', '9', temp]);
    await cp(temp, path);
    const dimensions = await probe(path);
    if (dimensions.width !== 2560 || dimensions.height !== 1600) throw new Error(`Unexpected still dimensions: ${JSON.stringify(dimensions)}`);
    produced.push(path);
  };
  const open = async title => {
    await keys('ctrl+p', '-t', title, '-s', '650', 'Return', '-s', '900');
  };
  const type = text => keys('--delay', '90', '-t', text);
  const paste = async code => {
    await writeFile(`${home}/solution.txt`, code);
    await boxRun('sh', '-c', 'wl-copy < "$HOME/solution.txt" > /dev/null 2>&1');
    await keys('ctrl+1', 'ctrl+a', 'ctrl+v', '-s', '350');
  };
  const start = async name => {
    const { stdout } = await boxRun('sh', '-c', `LIBVA_DRIVER_NAME=iHD wf-recorder ${recorderSettings.join(' ')} -f "$HOME/${name}.mkv" > "$HOME/recorder.log" 2>&1 & echo $!`);
    recorder = stdout.trim();
    if (!/^\d+$/.test(recorder)) throw new Error('Recorder did not return a PID');
    recordingStarted = performance.now();
    await sleep(250);
  };
  const stop = async name => {
    const wallDuration = (performance.now() - recordingStarted) / 1000;
    const pid = recorder;
    await boxRun('kill', '-INT', pid);
    recorder = null;
    for (let attempt = 0; attempt < 40; attempt++) {
      const { stdout } = await boxRun('sh', '-c', 'kill -0 "$1" 2>/dev/null && printf alive || printf done', 'recorder', pid);
      if (stdout === 'done') break;
      if (attempt === 39) throw new Error('Recorder did not finish within 10 seconds');
      await sleep(250);
    }
    const raw = `${masterDir ? destination : scratch}/${name}.mkv`;
    if (masterDir) {
      // wf-recorder's millisecond Matroska default duration reports 62.5 fps.
      // Stream-copy fixes the rate metadata without changing encoded RGB pixels or PTS.
      const recorded = `${scratch}/${name}.mkv`;
      await cp(`${home}/${name}.mkv`, recorded);
      await run('ffmpeg', ['-v', 'error', '-y', '-i', recorded, '-map', '0:v:0', '-c', 'copy', '-r', '60', raw]);
    } else await cp(`${home}/${name}.mkv`, raw);
    const measured = await cadence(raw, masterDir ? 60 : undefined);
    const rawInfo = await probe(raw);
    if (masterDir) {
      const { stdout } = await run('ffprobe', ['-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=codec_name,pix_fmt,color_space,profile', '-of', 'json', raw]);
      const format = JSON.parse(stdout).streams[0];
      if (rawInfo.width !== 2560 || rawInfo.height !== 1600 || rawInfo.fps !== 60 || format.pix_fmt !== 'gbrp') throw new Error(`Unexpected RGB master format: ${JSON.stringify({ ...rawInfo, ...format })}`);
      const log = `${destination}/${name}-recorder.log`;
      await copyFile(`${home}/recorder.log`, log);
      recordings.push({ name, ...rawInfo, ...format, recorderSettings, recordingWallDuration: wallDuration, cadence: measured });
      produced.push(raw, log);
      return;
    }
    const path = `${captures}/${name}.mp4`;
    await run('ffmpeg', ['-v', 'error', '-y', '-i', raw, '-an', '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-pix_fmt', 'yuv420p', '-r', '120', '-movflags', '+faststart', '-threads', '4', path]);
    recordings.push({ name, ...await probe(path), rawDuration: rawInfo.duration, recordingWallDuration: wallDuration, cadence: measured });
    produced.push(path);
  };
  try {
    // Codex is a static binary; the box needs its own copy because its HOME is not ours.
    const codex = codexHome ? ['--seed', `${await realpath((await run('sh', ['-c', 'command -v codex'])).stdout.trim())}:.local/bin/codex`] : [];
    await run('omabox', ['up', box, '--size', `2560x1600@${fps}`, '--no-shell', '--seed', `${homedir()}/.local/bin/leet:.local/bin/leet`, '--seed', `${homedir()}/.local/share/zed:.local/share/zed`, ...codex, '--json']);
    boxStarted = true;
    const { stdout } = await run('omabox', ['path', '-b', box]);
    home = `${stdout.trim()}/home`;
    await mkdir(`${home}/.config/leet`, { recursive: true });
    const { stdout: boxHome } = await boxRun('sh', '-c', 'printf %s "$HOME"');
    // --codex-home copies only that Codex login into the disposable box home, which `omabox down` deletes.
    const agent = codexHome ? 'agent = "codex"\n' : 'agent = "open-code"\n\n[[agent_models]]\nagent = "open-code"\nmodel = "opencode/mimo-v2.6-flash-free"\nfast = false\n';
    await writeFile(`${home}/.config/leet/config.toml`, `workspace = "${boxHome}/leet"\nonboarding_completed = true\n${agent}`);
    if (codexHome) {
      await mkdir(`${home}/.codex`, { recursive: true, mode: 0o700 });
      await copyFile(`${codexHome}/auth.json`, `${home}/.codex/auth.json`);
      await chmod(`${home}/.codex/auth.json`, 0o600);
    }
    await run('omabox', ['hyprctl', '-b', box, 'eval', `hl.monitor({output="HEADLESS-2",mode="2560x1600@${fps}",position="0x0",scale=2})`]);
    await run('omabox', ['run', '-b', box, '-d', '--', 'sh', '-c', 'exec env DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/leet-media-bus "$HOME/.local/bin/leet"']);
    await run('omabox', ['wait', '-b', box, 'window', 'leet', '--timeout', '20s']);
    await run('omabox', ['hyprctl', '-b', box, 'eval', 'hl.dispatch(hl.dsp.window.fullscreen({mode="fullscreen"}))']);
    // Let the startup companion-port warning disappear before any product footage.
    await sleep(10000);
    // Current installs start a guided tour in a fresh HOME; dismiss it for film capture.
    if (masterDir) await keys('Escape', '-s', '500');
    if (selected('flow')) {
      await start('flow');
      await keys('ctrl+p', '-t', 'two sum', '-s', '500');
      await shot('flow');
      await keys('Return', '-s', '700');
      await paste(twoSum);
      await keys('ctrl+Return', '-s', '4000');
      await stop('flow');
    } else {
      await open('two sum');
      await paste(twoSum);
      await keys('ctrl+Return', '-s', '1500');
    }
    if (selected('problem')) await shot('problem');
    if (selected('ai')) {
      await keys('alt+d', '-s', '500');
      await start('ai');
      await keys('ctrl+alt+5');
      // Observe only the throwaway cache, never a personal database.
      const deadline = Date.now() + 120000;
      const provider = codexHome ? 'Codex' : 'OpenCode free model';
      ai = { completed: false, provider };
      while (Date.now() < deadline) {
        await sleep(3000);
        const { stdout } = await run('sqlite3', [`${home}/.local/share/leet/leet.db`, 'SELECT body FROM chat_messages ORDER BY id DESC LIMIT 1']);
        if (stdout.trim()) {
          const turn = JSON.parse(stdout);
          if (turn.answer) { ai = { completed: true, provider, model: turn.model }; break; }
        }
        try {
          const log = await readFile(`${home}/.local/share/opencode/log/opencode.log`, 'utf8');
          if (log.includes('AI_RetryError')) {
            ai.error = log.includes('Rate limit exceeded') ? 'Rate limit exceeded' : log.includes('Endpoint is unavailable') ? 'Endpoint is unavailable' : 'OpenCode retries exhausted';
            break;
          }
        } catch (error) { if (error.code !== 'ENOENT') throw error; }
      }
      // Keep a completed answer readable in the roughly 30-second launch-film shot.
      if (masterDir && ai.completed) await sleep(Math.max(0, 27500 - (performance.now() - recordingStarted)));
      await sleep(2000);
      await shot('ai');
      await stop('ai');
      if (!ai.completed) warnings.push(`${provider} failed: ${ai.error ?? 'no answer within 120 seconds'}. AI still and source master retained; web video omitted.`);
      // Stop a pending request before capturing other views.
      if (!ai.completed) {
        const pending = `${scratch}/ai-pending.png`;
        await run('omabox', ['shot', '-b', box, '--window', 'leet', '-o', pending]);
        await run('omabox', ['click', '-b', box, '--in', pending, '2475', '1485']);
      }
      await keys('alt+d');
    }
    await open('container with most water');
    await paste(water);
    await keys('ctrl+Return', '-s', '1000');
    if (selected('debugger')) {
      await keys('alt+a', 'alt+x', 'alt+b', '-s', '1700');
      // Use the app's real 4x playback so several pointer moves fit in the short loop.
      const controls = `${scratch}/debugger-controls.png`;
      await run('omabox', ['shot', '-b', box, '--window', 'leet', '-o', controls]);
      await run('omabox', ['click', '-b', box, '--in', controls, '2490', '1495']);
      await run('omabox', ['click', '-b', box, '--in', controls, '2490', '1495']);
      await keys('Home', ...Array(4).fill('Right'));
      await shot('debugger');
      await start('debugger');
      await keys('Home', 'space', '-s', '2300');
      await keys('-s', '3000', 'Down', 'Home', 'space', '-s', '2400');
      await stop('debugger');
      await keys('alt+b', 'alt+a', 'alt+x');
    }
    if (filmSelected('film-search')) {
      await keys('ctrl+h', '-s', '800');
      await start('film-search');
      await keys('-s', '500', 'ctrl+p');
      await type('binary search');
      await keys('-s', '400', 'Down', 'Down', 'Return', '-s', '1200', 'ctrl+p');
      await type('trapping rain water');
      await keys('Return', '-s', '1500');
      await shot('film-search');
      await stop('film-search');
      await keys('ctrl+h');
    }
    if (filmSelected('film-panes')) {
      await open('two sum');
      await paste(twoSum);
      await start('film-panes');
      await keys('-s', '500', 'alt+s', '-s', '650', 'alt+a', '-s', '650', 'alt+d', '-s', '650', 'alt+s', '-s', '650', 'alt+a', '-s', '650', 'alt+d', '-s', '800');
      await shot('film-panes');
      await stop('film-panes');
    }
    if (filmSelected('film-debug')) {
      await open('container with most water');
      await paste(water);
      await keys('ctrl+Return', '-s', '1000');
      await keys('alt+s', 'alt+a', 'alt+x', 'alt+b', '-s', '1700', 'Home');
      // Fresh debug playback starts at 1x; film footage never touches the speed control.
      await start('film-debug');
      await keys('-s', '600', 'space', '-s', '6500', 'space');
      await shot('film-debug');
      await keys(...Array.from({ length: 12 }, () => ['Left', '-s', '120']).flat(), '-s', '600', 'Down', 'Home', 'space', '-s', '3000');
      await stop('film-debug');
      await keys('alt+b', 'alt+s', 'alt+a', 'alt+x');
    }
    await keys('ctrl+h', '-s', '700');
    if (selected('home')) await shot('home');
    if (selected('roadmap')) { await keys('alt+r', '-s', '600'); await shot('roadmap'); await keys('Escape', 'ctrl+h'); }
    if (selected('settings')) { await keys('ctrl+comma', '-s', '600'); await shot('settings'); await keys('Escape', 'ctrl+h'); }
    if (selected('palette')) { await keys('ctrl+k', '-s', '500'); await shot('palette'); }
    let previous = { recordings: [] };
    try { previous = JSON.parse(await readFile(`${destination}/capture.json`, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    const metadata = { environment: 'omabox', source: 'installed leet, isolated HOME and blocked keyring', recordings: [...previous.recordings.filter(old => !recordings.some(item => item.name === old.name)), ...recordings], ai: ai ?? previous.ai, warnings: ai ? warnings : previous.warnings ?? warnings };
    await writeFile(`${destination}/capture.json`, `${JSON.stringify(metadata, null, 2)}\n`);
    produced.push(`${destination}/capture.json`);
    return { command: 'media:capture', environment: 'omabox', success: true, files: await files(produced), recordings, warnings };
  } finally {
    process.removeListener('SIGINT', interrupt);
    process.removeListener('SIGTERM', interrupt);
    if (recorder) await boxRun('kill', '-INT', recorder).catch(() => {});
    if (boxStarted) await run('omabox', ['down', box]);
    if (masterDir) await rm(scratch, { recursive: true, force: true });
  }
}
