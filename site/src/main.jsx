import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Kbd, KbdGroup } from './components/ui/kbd.jsx';
import { detectPlatform, installCommand, requirements } from './platform.js';
import { shouldCopyInstallCommand } from './shortcut.js';
import './style.css';

function LandingPage() {
  const detected = detectPlatform(navigator.userAgentData?.platform ?? navigator.platform, navigator.userAgent);
  const [platform, setPlatform] = useState(detected === 'windows' ? 'windows' : 'unix');
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState(false);
  const command = useRef(null);

  async function copyCommand() {
    try {
      await navigator.clipboard.writeText(installCommand);
      setCopied(true);
      setCopyError(false);
    } catch {
      const range = document.createRange();
      range.selectNodeContents(command.current);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      setCopyError(true);
    }
  }

  useEffect(() => {
    function onKeyDown(event) {
      if (!shouldCopyInstallCommand({
        key: event.key, ctrlKey: event.ctrlKey, metaKey: event.metaKey,
        altKey: event.altKey, shiftKey: event.shiftKey, repeat: event.repeat,
        editable: event.target instanceof HTMLElement && (event.target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(event.target.tagName)),
        selection: window.getSelection()?.toString() ?? '', platform,
      })) return;
      event.preventDefault();
      void copyCommand();
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [platform]);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1800);
    return () => clearTimeout(timer);
  }, [copied]);

  return (
    <main>
      <span className="brand"><img src={`${import.meta.env.BASE_URL}leet.svg`} alt="" width="40" height="40" />leet</span>
      <div className="intro">
        <p className="eyebrow">Native coding practice</p>
        <h1>leet for <span>eleet</span></h1>
        <p className="description">Fully Rust. Fast. Smooth.</p>
        <p className="providers">NeetCode, LeetCode &amp; Codeforces.</p>
      </div>
      <section className="install" aria-label="Get leet">
        <div className="install-heading">
          <h2>Get leet</h2>
          <label className="platform-label">Platform
            <select id="platform" aria-label="Platform" value={platform} onChange={event => {
              setPlatform(event.target.value);
              setCopied(false);
              setCopyError(false);
            }}>
              <option value="unix">Linux &amp; macOS</option>
              <option value="windows">Windows</option>
            </select>
          </label>
        </div>
        {platform === 'unix' ? (
          <div className="command-row" id="unix-install">
            <code id="command" ref={command}>{installCommand}</code>
            <button id="copy" className="copy-button" type="button" onClick={() => void copyCommand()} aria-keyshortcuts="Control+C Meta+C" aria-live="polite">
              <span>{copied ? 'Copied' : 'Copy'}</span>
              <KbdGroup aria-hidden="true"><Kbd>Ctrl</Kbd><Kbd>C</Kbd></KbdGroup>
            </button>
          </div>
        ) : (
          <a id="download" className="download" href="https://github.com/Noelithub77/leet/releases/latest/download/leet-windows-x86_64.exe">Download .exe <span aria-hidden="true">↓</span></a>
        )}
        {copyError && <p className="copy-error" role="status">Press Ctrl+C to copy the selected command.</p>}
        <details>
          <summary>Requirements &amp; releases</summary>
          <p>{requirements[platform]}</p>
          <p>Unsigned builds may show OS security prompts. Local solutions need Python or a compiler.</p>
          <a href="https://github.com/Noelithub77/leet/releases">All releases ↗</a>
        </details>
      </section>
      <footer><span>Open source · keyboard first</span><a href="https://github.com/Noelithub77/leet">Source ↗</a></footer>
    </main>
  );
}

createRoot(document.getElementById('root')).render(<LandingPage />);
