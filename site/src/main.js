import './style.css';
import { detectPlatform, installCommand, requirements } from './platform.js';

const platform = document.querySelector('#platform');
const status = document.querySelector('#status');
const command = document.querySelector('#command');
const copy = document.querySelector('#copy');
command.textContent = installCommand;
const detected = detectPlatform(navigator.userAgentData?.platform ?? navigator.platform, navigator.userAgent);
platform.value = detected ?? 'linux';

function updatePlatform() {
  document.querySelector('#unix-install').hidden = platform.value === 'windows';
  document.querySelector('#windows-install').hidden = platform.value !== 'windows';
  document.querySelector('#requirements').textContent = requirements[platform.value];
  status.textContent = '';
  copy.textContent = 'Copy';
}
platform.addEventListener('change', updatePlatform);
updatePlatform();
if (!detected) status.textContent = 'Choose the platform of your desktop computer.';

copy.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(installCommand);
    copy.textContent = 'Copied';
    status.textContent = 'Ready to paste in your terminal.';
  } catch {
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(command);
    selection?.removeAllRanges();
    selection?.addRange(range);
    status.textContent = 'Press Ctrl+C or ⌘C to copy the selected command.';
  }
});
