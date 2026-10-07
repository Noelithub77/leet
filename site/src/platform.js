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
  linux: 'Linux x86_64 or ARM64 with a graphical desktop and Vulkan drivers. The installer can install runtime libraries on Ubuntu/Debian, Fedora, Arch, and openSUSE; sudo may ask for your password.',
  macos: 'macOS 13 or later. Apple Silicon and Intel builds are selected automatically. Installs in your Applications folder.',
  windows: 'Windows 10 or later, x64. The portable executable saves settings and solutions in your user folders.',
};
