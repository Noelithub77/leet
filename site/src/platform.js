export const installCommand = 'curl -fsSL https://noelithub77.github.io/leet/install.sh | sh';

export function detectPlatform(platform, userAgent) {
  const value = `${platform} ${userAgent}`;
  if (/Android|iPhone|iPad|iPod/i.test(value)) return null;
  if (/Win/i.test(value)) return 'windows';
  if (/Mac/i.test(value)) return 'macos';
  if (/Linux/i.test(value)) return 'linux';
  return null;
}

export const requirements = {
  unix: 'Linux x86_64/ARM64: graphical desktop and Vulkan drivers. macOS 13+: Intel or Apple Silicon. The installer selects your build; Linux runtime libraries may need sudo.',
  windows: 'Windows 10 or later, x64. The portable executable saves settings and solutions in your user folders.',
};
