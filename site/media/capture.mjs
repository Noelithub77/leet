import { mkdir, cp, copyFile, chmod, realpath, writeFile, readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { captures, mediaDir, run, sleep, files, probe, cadence } from './process.mjs';

export const sceneNames = ['flow', 'debugger', 'ai', 'home', 'roadmap', 'settings', 'palette', 'problem'];
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

export async function capture(scene, { codexHome } = {}) {
  if (scene && !sceneNames.includes(scene)) throw new Error(`Unknown scene ${scene}; choose ${sceneNames.join(', ')}`);
  await mkdir(captures, { recursive: true });
  await mkdir(`${mediaDir}.scratch`, { recursive: true });
  const box = `leet-media-${process.pid}`;
  const boxRun = (...args) => run('omabox', ['run', '-b', box, '--', ...args]);
  const keys = (...args) => run('omabox', ['keys', '-b', box, '--window', 'leet', ...args]);
  const selected = name => !scene || scene === name;
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
    const path = `${captures}/${name}.png`;
    await run('omabox', ['shot', '-b', box, '--window', 'leet', '-o', path]);
    // Recompress losslessly; PNG compression_level ranges from 0 to 9 in FFmpeg.
    const temp = `${mediaDir}.scratch/${name}.png`;
    await run('ffmpeg', ['-v', 'error', '-y', '-i', path, '-compression_level', '9', temp]);
    await cp(temp, path);
    const dimensions = await probe(path);
    if (dimensions.width !== 2560 || dimensions.height !== 1600) throw new Error(`Unexpected still dimensions: ${JSON.stringify(dimensions)}`);
    produced.push(path);
  };
  const open = async title => {
    await keys('ctrl+p', '-t', title, '-s', '650', 'Return', '-s', '900');
  };
  const paste = async code => {
    await writeFile(`${home}/solution.txt`, code);
    await boxRun('sh', '-c', 'wl-copy < "$HOME/solution.txt" > /dev/null 2>&1');
    await keys('ctrl+1', 'ctrl+a', 'ctrl+v', '-s', '350');
  };
  const start = async name => {
    const { stdout } = await boxRun('sh', '-c', `LIBVA_DRIVER_NAME=iHD wf-recorder -r 120 -c h264_vaapi -p qp=16 -d /dev/dri/renderD129 -f "$HOME/${name}.mkv" > "$HOME/recorder.log" 2>&1 & echo $!`);
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
    const raw = `${mediaDir}.scratch/${name}.mkv`;
    await cp(`${home}/${name}.mkv`, raw);
    const measured = await cadence(raw);
    const path = `${captures}/${name}.mp4`;
    const rawInfo = await probe(raw);
    await run('ffmpeg', ['-v', 'error', '-y', '-i', raw, '-an', '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-pix_fmt', 'yuv420p', '-r', '120', '-movflags', '+faststart', '-threads', '4', path]);
    recordings.push({ name, ...await probe(path), rawDuration: rawInfo.duration, recordingWallDuration: wallDuration, cadence: measured });
    produced.push(path);
  };
  try {
    // Codex is a static binary; the box needs its own copy because its HOME is not ours.
    const codex = codexHome ? ['--seed', `${await realpath((await run('sh', ['-c', 'command -v codex'])).stdout.trim())}:.local/bin/codex`] : [];
    await run('omabox', ['up', box, '--size', '2560x1600@120', '--no-shell', '--seed', `${homedir()}/.local/bin/leet:.local/bin/leet`, '--seed', `${homedir()}/.local/share/zed:.local/share/zed`, ...codex, '--json']);
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
    await run('omabox', ['hyprctl', '-b', box, 'eval', 'hl.monitor({output="HEADLESS-2",mode="2560x1600@120",position="0x0",scale=2})']);
    await run('omabox', ['run', '-b', box, '-d', '--', 'sh', '-c', 'exec env DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/leet-media-bus "$HOME/.local/bin/leet"']);
    await run('omabox', ['wait', '-b', box, 'window', 'leet', '--timeout', '20s']);
    await run('omabox', ['hyprctl', '-b', box, 'eval', 'hl.dispatch(hl.dsp.window.fullscreen({mode="fullscreen"}))']);
    // Let the startup companion-port warning disappear before any product footage.
    await sleep(10000);
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
      await sleep(2000);
      await shot('ai');
      await stop('ai');
      if (!ai.completed) warnings.push(`${provider} failed: ${ai.error ?? 'no answer within 120 seconds'}. AI still and source master retained; web video omitted.`);
      // Stop a pending request before capturing other views.
      if (!ai.completed) {
        const pending = `${mediaDir}.scratch/ai-pending.png`;
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
      const controls = `${mediaDir}.scratch/debugger-controls.png`;
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
    await keys('ctrl+h', '-s', '700');
    if (selected('home')) await shot('home');
    if (selected('roadmap')) { await keys('alt+r', '-s', '600'); await shot('roadmap'); await keys('Escape', 'ctrl+h'); }
    if (selected('settings')) { await keys('ctrl+comma', '-s', '600'); await shot('settings'); await keys('Escape', 'ctrl+h'); }
    if (selected('palette')) { await keys('ctrl+k', '-s', '500'); await shot('palette'); }
    let previous = { recordings: [] };
    try { previous = JSON.parse(await readFile(`${captures}/capture.json`, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    const metadata = { environment: 'omabox', source: 'installed leet, isolated HOME and blocked keyring', recordings: [...previous.recordings.filter(old => !recordings.some(item => item.name === old.name)), ...recordings], ai: ai ?? previous.ai, warnings: ai ? warnings : previous.warnings ?? warnings };
    await writeFile(`${captures}/capture.json`, `${JSON.stringify(metadata, null, 2)}\n`);
    produced.push(`${captures}/capture.json`);
    return { command: 'media:capture', environment: 'omabox', success: true, files: await files(produced), recordings, warnings };
  } finally {
    process.removeListener('SIGINT', interrupt);
    process.removeListener('SIGTERM', interrupt);
    if (recorder) await boxRun('kill', '-INT', recorder).catch(() => {});
    if (boxStarted) await run('omabox', ['down', box]);
  }
}
