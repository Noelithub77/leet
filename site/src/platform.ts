export const installCommand = 'curl -fsSL https://leet.allpyq.in/install.sh | sh';

export function detectPlatform(platform, userAgent) {
  const value = `${platform} ${userAgent}`;
  if (/Android|iPhone|iPad|iPod/i.test(value)) return null;
  if (/Win/i.test(value)) return 'windows';
  if (/Mac/i.test(value)) return 'macos';
  if (/Linux/i.test(value)) return 'linux';
  return null;
}
