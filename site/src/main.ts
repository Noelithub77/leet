import './style.css';
import { detectPlatform, installCommand } from './platform';
import { shouldCopyInstallCommand } from './shortcut';

const detected = detectPlatform(navigator.userAgentData?.platform ?? navigator.platform, navigator.userAgent);
const modifier = detected === 'macos' ? '⌘' : 'Ctrl';
let platform = detected === 'windows' ? 'windows' : 'unix';

for (const key of document.querySelectorAll('[data-mod]')) {
  key.textContent = modifier;
  key.classList.toggle('is-mac', detected === 'macos');
  if (detected === 'macos') key.setAttribute('aria-label', 'Command');
}
// Phones and tablets have no Ctrl+C to advertise.
if (!detected) document.querySelector('[data-press-hint]')?.setAttribute('hidden', '');

const notices = () => import('./toast');

function setPlatform(next) {
  platform = next;
  for (const tab of document.querySelectorAll('[data-platform]')) tab.setAttribute('aria-selected', String(tab.dataset.platform === next));
  for (const panel of document.querySelectorAll('[data-panel]')) panel.hidden = panel.dataset.panel !== next;
}

function visibleCopyButton() {
  const buttons = [...document.querySelectorAll('[data-copy]')];
  return buttons.find(button => {
    const { top, bottom } = button.getBoundingClientRect();
    return bottom > 0 && top < innerHeight;
  }) ?? buttons[0];
}

const check = '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8.5l3.2 3L13 4.5"/></svg>';
const timers = new WeakMap();

function celebrate(button) {
  const row = button.closest('.command-row');
  const label = button.querySelector('[data-copy-label]');
  label.innerHTML = `${check}Copied`;
  row.classList.add('is-copied');
  clearTimeout(timers.get(button));
  timers.set(button, setTimeout(() => {
    label.textContent = '';
    row.classList.remove('is-copied');
  }, 1800));
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  for (let index = 0; index < 14; index += 1) {
    const spark = document.createElement('i');
    spark.className = 'spark';
    spark.style.background = index % 3 ? 'var(--mint)' : 'var(--peach)';
    button.append(spark);
    const angle = (index / 14) * Math.PI * 2 + Math.random() * 0.4;
    const distance = 34 + Math.random() * 34;
    spark.animate([
      { transform: 'translate(-50%, -50%) scale(1)', opacity: 1 },
      { transform: `translate(calc(-50% + ${Math.cos(angle) * distance}px), calc(-50% + ${Math.sin(angle) * distance}px)) scale(0) rotate(${angle * 90}deg)`, opacity: 0 },
    ], { duration: 650 + Math.random() * 250, easing: 'cubic-bezier(.22,1,.36,1)' }).finished.then(() => spark.remove());
  }
}

async function copy(button, source) {
  try {
    await navigator.clipboard.writeText(installCommand);
  } catch {
    const range = document.createRange();
    range.selectNodeContents(button.closest('.command-row').querySelector('[data-command]'));
    getSelection()?.removeAllRanges();
    getSelection()?.addRange(range);
    (await notices()).selectFallback(modifier);
    return;
  }
  celebrate(button);
  (await notices()).copied(source, modifier);
}

document.addEventListener('click', event => {
  const target = event.target instanceof Element ? event.target : null;
  const tab = target?.closest('[data-platform]');
  if (tab) setPlatform(tab.dataset.platform);
  const button = target?.closest('[data-copy]');
  if (button) void copy(button, 'click');
  if (target?.closest('.download')) void notices().then(toast => toast.downloading());
});

document.addEventListener('keydown', event => {
  if (!shouldCopyInstallCommand({
    key: event.key, ctrlKey: event.ctrlKey, metaKey: event.metaKey,
    altKey: event.altKey, shiftKey: event.shiftKey, repeat: event.repeat,
    editable: event.target instanceof HTMLElement && (event.target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(event.target.tagName)),
    selection: getSelection()?.toString() ?? '', platform,
  })) return;
  event.preventDefault();
  const button = visibleCopyButton();
  for (const key of button.querySelectorAll('kbd')) {
    key.classList.add('is-pressed');
    setTimeout(() => key.classList.remove('is-pressed'), 160);
  }
  void copy(button, 'keyboard');
});

setPlatform(platform);
// Let the command paint before requesting the motion libraries.
requestAnimationFrame(() => {
  (window.requestIdleCallback ?? (callback => setTimeout(callback, 100)))(() => void import('./enhance'), { timeout: 1800 });
});
// Toasts need React; warm them up on idle unless the device is constrained.
if (!navigator.connection?.saveData && (navigator.hardwareConcurrency ?? 4) >= 4) {
  (window.requestIdleCallback ?? (callback => setTimeout(callback, 1500)))(() => void notices(), { timeout: 4000 });
}
