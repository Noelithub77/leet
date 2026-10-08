// Loaded after first paint: React only powers Sonner's toasts.
import React from 'react';
import { createRoot } from 'react-dom/client';
import { Toaster, toast } from 'sonner';
import { colors, logo, pointsAttribute } from './logo';

function Mark() {
  return (
    <svg viewBox="0 0 64 64" width="20" height="20" aria-hidden="true">
      {logo.map((stroke, index) => (
        <polyline key={index} points={pointsAttribute(stroke.points)} fill="none" stroke={colors[index]} strokeWidth={stroke.width} strokeLinecap="round" strokeLinejoin="round" />
      ))}
    </svg>
  );
}

const host = document.createElement('div');
document.body.append(host);
createRoot(host).render(
  <Toaster
    theme="dark"
    position="bottom-center"
    offset={24}
    mobileOffset={16}
    gap={10}
    visibleToasts={3}
    style={{ '--normal-bg': '#161616', '--normal-border': '#2c2c2c', '--normal-text': '#e6e6e6', '--border-radius': '14px', fontFamily: 'inherit' }}
    toastOptions={{ className: 'leet-toast' }}
  />,
);

export function copied(source, modifier) {
  toast('Install command copied', {
    id: 'copy',
    icon: <Mark />,
    description: source === 'keyboard'
      ? `${modifier} C works anywhere here. Paste it into a terminal.`
      : 'Paste it into a terminal. leet installs and launches.',
  });
}

export function selectFallback(modifier) {
  toast('Command selected', { id: 'copy', icon: <Mark />, description: `Clipboard access is blocked. Press ${modifier} C to copy.` });
}

export function downloading() {
  toast('Downloading leet for Windows', { id: 'download', icon: <Mark />, description: 'Open the .exe. The first releases are unsigned.' });
}

export function eleet(on) {
  toast(on ? 'Eleet mode' : 'Back to leet', { id: 'eleet', icon: <Mark />, description: on ? '1337 @ 120Hz. Type it again to switch back.' : 'Colors restored.' });
}
