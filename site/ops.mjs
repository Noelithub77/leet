import { spawnSync } from 'node:child_process';

const [command = '--help', ...args] = process.argv.slice(2);
if (command === '--help') {
  console.log('pnpm ops <check|build> [--json]\n\nLocal landing page checks or production build. Requires pnpm install.');
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
