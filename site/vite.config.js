import { readFileSync } from 'node:fs';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

// Serve the desktop app's logo as /leet/leet.svg so both share one file.
const logo = new URL('../crates/gui/assets/leet.svg', import.meta.url);
const appLogo = {
  name: 'app-logo',
  configureServer(server) {
    server.middlewares.use('/leet/leet.svg', (_request, response) => {
      response.setHeader('Content-Type', 'image/svg+xml');
      response.end(readFileSync(logo));
    });
  },
  generateBundle() {
    this.emitFile({ type: 'asset', fileName: 'leet.svg', source: readFileSync(logo) });
  },
};

export default defineConfig({ base: '/leet/', plugins: [tailwindcss(), appLogo] });
