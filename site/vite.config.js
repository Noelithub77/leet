import { readFileSync } from 'node:fs';
import { createLogger, defineConfig } from 'vite';

// The site reuses the desktop app's SVGs so each mark has one source file.
const assets = new URL('../crates/gui/assets/', import.meta.url);
const shared = {
  'leet.svg': 'leet.svg',
  'brand/LICENSE': 'providers/LICENSE',
  'brand/README.md': 'providers/README.md',
  ...Object.fromEntries(['codex', 'claude-code', 'opencode', 'antigravity', 'gemini-cli', 'cursor', 'leetcode', 'codeforces', 'codechef', 'neetcode']
    .map(name => [`brand/${name}.svg`, `providers/${name}.svg`])),
};
const appAssets = {
  name: 'app-assets',
  configureServer(server) {
    server.middlewares.use((request, response, next) => {
      const source = shared[request.url?.replace(/^\//, '').split('?')[0]];
      if (!source) return next();
      response.setHeader('Content-Type', source.endsWith('.svg') ? 'image/svg+xml' : 'text/plain; charset=utf-8');
      response.end(readFileSync(new URL(source, assets)));
    });
  },
  generateBundle() {
    for (const [fileName, source] of Object.entries(shared)) {
      this.emitFile({ type: 'asset', fileName, source: readFileSync(new URL(source, assets)) });
    }
  },
};

// Inline-style url()s for fonts and brand marks resolve from the site root.
const logger = createLogger();
const warn = logger.warn;
const runtimeAssets = [...Object.keys(shared).map(name => `/${name}`),
  '/fonts/liberation-sans-regular.woff2', '/fonts/liberation-sans-bold.woff2', '/fonts/nunito-bold.woff2'];
logger.warn = (message, options) => {
  const expected = runtimeAssets.some(path => message.startsWith(`${path} referenced in ${path} didn't resolve at build time`));
  if (!expected) warn(message, options);
};
logger.warnOnce = logger.warn;

export default defineConfig({
  base: '/',
  server: { port: 1337, strictPort: true },
  customLogger: logger,
  plugins: [appAssets],
  build: {
    target: 'es2022',
    modulePreload: { polyfill: false },
    rolldownOptions: {
      // Sonner's 'use client' marker is irrelevant outside React Server Components.
      onLog(level, log, handler) { if (log.code !== 'MODULE_LEVEL_DIRECTIVE') handler(level, log); },
    },
  },
});
