import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Kbd, KbdGroup } from './components/ui/kbd.jsx';
import { detectPlatform, installCommand } from './platform.js';
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
      <header>
        <span className="brand"><img src={`${import.meta.env.BASE_URL}leet.svg`} alt="" width="40" height="40" />leet</span>
        <a className="github-link" href="https://github.com/Noelithub77/leet" aria-label="leet on GitHub">
          <svg width="28" height="28" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M6.766 11.328c-2.063-.25-3.516-1.734-3.516-3.656 0-.781.281-1.625.75-2.188-.203-.515-.172-1.609.063-2.062.625-.078 1.468.25 1.968.703.594-.187 1.219-.281 1.985-.281.765 0 1.39.094 1.953.265.484-.437 1.344-.765 1.969-.687.218.422.25 1.515.046 2.047.5.593.766 1.39.766 2.203 0 1.922-1.453 3.375-3.547 3.64.531.344.89 1.094.89 1.954v1.625c0 .468.391.734.86.547C13.781 14.359 16 11.53 16 8.03 16 3.61 12.406 0 7.984 0 3.563 0 0 3.61 0 8.031a7.88 7.88 0 0 0 5.172 7.422c.422.156.828-.125.828-.547v-1.25c-.219.094-.5.156-.75.156-1.031 0-1.64-.562-2.078-1.609-.172-.422-.36-.672-.719-.719-.187-.015-.25-.093-.25-.187 0-.188.313-.328.625-.328.453 0 .844.281 1.25.86.313.452.64.655 1.031.655s.641-.14 1-.5c.266-.265.47-.5.657-.656" />
          </svg>
        </a>
      </header>
      <div className="intro">
        <p className="eyebrow">Native coding practice</p>
        <h1>leet for the <span>eleet</span></h1>
        <p className="description">100% idiomatic rust, gpui, leet at 120hz</p>
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
      </section>
    </main>
  );
}

createRoot(document.getElementById('root')).render(<LandingPage />);
