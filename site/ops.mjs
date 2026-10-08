import { spawnSync } from 'node:child_process';
import { capture, sceneNames } from './media/capture.mjs';
import { encode } from './media/encode.mjs';

const [command = '--help', ...args] = process.argv.slice(2);
if (command === '--help') {
  console.log('pnpm ops <check|build|media:capture|media:encode> [--json]\n\nLocal landing page checks or production build. Requires pnpm install.\nUse pnpm ops media --help for landing media commands.');
} else if (command === 'media' || command.startsWith('media:')) {
  const help = `pnpm ops media:capture [--scene NAME] [--codex-home PATH] [--json]\npnpm ops media:encode [--json]\n\nScenes: ${sceneNames.join(', ')}\nCapture uses the installed leet executable inside a disposable OmaBox at 2560x1600@120, scale 2.\nRequires omabox, wf-recorder, Intel VAAPI (/dev/dri/renderD129), ffmpeg/ffprobe with x264, SVT-AV1, libaom and WebP, wl-copy, sqlite3, and ~/.local/share/zed.\nSource PNGs and 120 fps masters: media/captures/. Web assets: public/media/.\nThe AI scene waits up to 120 seconds for a live OpenCode free model, or for Codex when --codex-home names a Codex home whose auth.json is copied into the disposable box only; incomplete AI video is omitted.\nEncoding needs only the tracked captures and ffmpeg; it does not launch a desktop.`;
  try {
    if (args.includes('--help') && args.length === 1) console.log(help);
    else {
      let scene;
      let codexHome;
      for (let i = 0; i < args.length; i++) {
        if (args[i] === '--json') continue;
        if (command === 'media:capture' && args[i] === '--scene' && args[i + 1]) { scene = args[++i]; continue; }
        if (command === 'media:capture' && args[i] === '--codex-home' && args[i + 1]) { codexHome = args[++i]; continue; }
        throw new Error(`Unknown argument ${args[i]}\n${help}`);
      }
      if (!['media:capture', 'media:encode'].includes(command)) throw new Error(help);
      console.log(JSON.stringify(command === 'media:capture' ? await capture(scene, { codexHome }) : await encode()));
    }
  } catch (error) {
    console.error(JSON.stringify({ command, environment: 'omabox', success: false, error: error.message }));
    process.exitCode = 1;
  }
} else {
  if (!['check', 'build'].includes(command) || args.some(arg => arg !== '--json')) {
    console.error('Use pnpm ops --help');
    process.exit(1);
  }
  for (const script of command === 'check' ? ['test', 'build'] : ['build']) {
    const result = spawnSync('pnpm', ['run', script], { stdio: ['ignore', 2, 2] });
    if (result.error || result.status !== 0) {
      console.error(result.error?.message ?? `${script} failed`);
      process.exit(1);
    }
  }
  console.log(JSON.stringify({ command, environment: 'local-site', success: true }));
}
