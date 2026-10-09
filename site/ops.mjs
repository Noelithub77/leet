import { spawnSync } from 'node:child_process';
import { capture, sceneNames, filmSceneNames } from './media/capture.mjs';
import { encode, encodeReadme } from './media/encode.mjs';

const [command = '--help', ...args] = process.argv.slice(2);
if (command === '--help') {
  console.log('pnpm ops <check|build|media:capture|media:encode> [--json]\n\nLocal landing page checks or production build. Requires pnpm install.\nUse pnpm ops media --help for landing media commands.');
} else if (command === 'media' || command.startsWith('media:')) {
  const help = `pnpm ops media:capture [--master-dir PATH] [--scene NAME] [--codex-home PATH] [--json]\npnpm ops media:encode [--json]\npnpm ops media:readme [--json]\n\nScenes: ${sceneNames.join(', ')}\nFilm scenes (require --master-dir; included when no --scene is supplied): ${filmSceneNames.join(', ')}\nCapture uses the installed leet executable inside a disposable OmaBox at 2560x1600@120, scale 2.\nRequires omabox, wf-recorder, Intel VAAPI (/dev/dri/renderD129), ffmpeg/ffprobe with x264, SVT-AV1, libaom and WebP, wl-copy, sqlite3, and ~/.local/share/zed.\nSource PNGs and 120 fps masters: media/captures/. Web assets: public/media/.\n--master-dir PATH records the same scenes at 60 Hz/fps with lossless libx264rgb (bgr0, preset ultrafast, crf 0, 4 threads). Raw MKVs, PNGs, recorder logs and capture.json go only to PATH; no web encoding or published asset writes. This mode needs CPU x264 RGB instead of Intel VAAPI.\nThe AI scene waits up to 120 seconds for a live OpenCode free model, or for Codex when --codex-home names a Codex home whose auth.json is copied into the disposable box only; incomplete AI video is omitted.\nEncoding needs only the tracked captures and ffmpeg; it does not launch a desktop.\nmedia:readme writes lossless looping WebP previews to docs/assets/ from the original masters, using full width for Workspace and 1280px for the paired demos.`;
  try {
    if (args.includes('--help') && args.length === 1) console.log(help);
    else {
      let scene;
      let codexHome;
      let masterDir;
      for (let i = 0; i < args.length; i++) {
        if (args[i] === '--json') continue;
        if (command === 'media:capture' && args[i] === '--scene' && args[i + 1]) { scene = args[++i]; continue; }
        if (command === 'media:capture' && args[i] === '--codex-home' && args[i + 1]) { codexHome = args[++i]; continue; }
        if (command === 'media:capture' && args[i] === '--master-dir' && args[i + 1]) { masterDir = args[++i]; continue; }
        throw new Error(`Unknown argument ${args[i]}\n${help}`);
      }
      if (!['media:capture', 'media:encode', 'media:readme'].includes(command)) throw new Error(help);
      console.log(JSON.stringify(command === 'media:capture' ? await capture(scene, { codexHome, masterDir }) : command === 'media:readme' ? await encodeReadme() : await encode()));
    }
  } catch (error) {
    console.error(JSON.stringify({ command, environment: command === 'media:readme' ? 'local-media' : 'omabox', success: false, error: error.message }));
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
