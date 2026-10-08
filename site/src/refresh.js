const RATES = [30, 48, 50, 60, 72, 75, 90, 100, 120, 144, 165, 180, 240, 360];

/** The display refresh rate implied by animation-frame intervals in milliseconds. */
export function snapRefreshRate(deltas) {
  const usable = deltas.filter(delta => delta > 2 && delta < 50).sort((a, b) => a - b);
  if (usable.length < 8) return null;
  const median = usable[Math.floor(usable.length / 2)];
  const measured = 1000 / median;
  return RATES.reduce((best, rate) => (Math.abs(rate - measured) < Math.abs(best - measured) ? rate : best));
}
