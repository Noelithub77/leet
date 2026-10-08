import { gsap } from 'gsap';
import { lineHeat, maxAreaTrace, sample, source } from './trace';

const SVG = 'http://www.w3.org/2000/svg';
const STEP_SECONDS = 0.42;
const LEFT = 22;
const SLOT = 37.5;
const BASE = 168;
const UNIT = 17;

const keywords = /\b(class|def|while|if|else|return|in)\b/g;
const calls = /\b(maxArea|min|max|len)\b/g;
const numbers = /\b(\d+)\b/g;
const escape = text => text.replace(/[&<>]/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[char]);
const highlight = line => escape(line).replace(keywords, '<span class="kw">$1</span>').replace(calls, '<span class="fn">$1</span>').replace(numbers, '<span class="num">$1</span>');
const center = index => LEFT + SLOT * index + SLOT / 2;

function node(name, attributes, parent) {
  const element = document.createElementNS(SVG, name);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
  parent?.append(element);
  return element;
}

/** Interactive replay. Returns controls so a pinned scroll scene can drive it. */
export function mountDebugger(card, { reduced }) {
  const steps = maxAreaTrace(sample);
  const code = card.querySelector('[data-code]');
  const viz = card.querySelector('[data-viz]');
  const vars = card.querySelector('[data-vars]');
  const seek = card.querySelector('[data-seek]');
  const play = card.querySelector('[data-play]');
  const stepLabel = card.querySelector('[data-step]');
  card.querySelector('[data-total]').textContent = String(steps.length);
  seek.max = String(steps.length - 1);

  // The best container so far, for the dashed outline.
  let pair = null;
  const bestPairs = steps.map((step, index) => {
    const previous = steps[index - 1];
    if (previous && step.best !== undefined && step.best > (previous.best ?? 0)) pair = [previous.l, previous.r];
    return pair;
  });

  code.innerHTML = source.map(line => `<li><i class="heat"></i><span>${highlight(line)}</span><span class="hits"></span></li>`).join('');
  const lines = [...code.children];
  const water = node('rect', { fill: '#99ffe4', 'fill-opacity': '.13', rx: '3' }, viz);
  const surface = node('line', { stroke: '#99ffe4', 'stroke-width': '2', 'stroke-linecap': 'round', 'stroke-opacity': '.8' }, viz);
  const best = node('rect', { fill: 'none', stroke: '#ffc799', 'stroke-width': '1.4', 'stroke-dasharray': '4 4', rx: '3', opacity: '0' }, viz);
  const bestLabel = node('text', { class: 'best-label', opacity: '0' }, viz);
  node('line', { x1: LEFT - 6, x2: LEFT + SLOT * sample.length + 6, y1: BASE, y2: BASE, stroke: '#2e2e2e' }, viz);
  const bars = sample.map((height, index) => {
    const bar = node('rect', { x: center(index) - 11, width: '22', y: BASE - height * UNIT, height: height * UNIT, rx: '5' }, viz);
    const value = node('text', { x: center(index), y: BASE - height * UNIT + 13, class: 'val' }, viz);
    value.textContent = String(height);
    node('text', { x: center(index), y: BASE + 16 }, viz).textContent = String(index);
    return { bar, value };
  });
  const pointer = label => {
    const group = node('g', {}, viz);
    node('path', { d: 'M0 0 l-5 8 h10 z', fill: '#99ffe4' }, group);
    node('line', { x1: '0', x2: '0', y1: '8', y2: '24', stroke: '#99ffe4', 'stroke-width': '2' }, group);
    node('text', { x: '0', y: '40', class: 'ptr' }, group).textContent = label;
    return group;
  };
  const pointers = { l: pointer('l'), r: pointer('r') };

  const shown = { l: 0, r: sample.length - 1, level: 0 };
  const geometry = () => {
    const x1 = center(shown.l);
    const x2 = center(shown.r);
    const top = BASE - shown.level;
    gsap.set(water, { attr: { x: x1, width: Math.max(0, x2 - x1), y: top, height: shown.level } });
    gsap.set(surface, { attr: { x1, x2, y1: top, y2: top } });
    pointers.l.setAttribute('transform', `translate(${x1} ${BASE + 24})`);
    pointers.r.setAttribute('transform', `translate(${x2} ${BASE + 24})`);
  };

  let index = 0;
  const render = () => {
    const step = steps[index];
    const previous = steps[index - 1] ?? {};
    stepLabel.textContent = String(index + 1);
    seek.value = String(index);
    seek.style.setProperty('--p', `${(index / (steps.length - 1)) * 100}%`);
    const heat = lineHeat(steps, index);
    const most = Math.max(...heat, 1);
    lines.forEach((line, lineIndex) => {
      const count = heat[lineIndex + 1];
      line.classList.toggle('is-current', step.line === lineIndex + 1);
      line.firstChild.style.opacity = count ? String(0.25 + (count / most) * 0.75) : '0';
      line.lastChild.textContent = count ? `×${count}` : '';
    });
    vars.innerHTML = ['l', 'r', 'area', 'best'].filter(name => step[name] !== undefined)
      .map(name => `<span class="${previous[name] !== step[name] ? 'is-changed' : ''}">${name} <b>${step[name]}</b></span>`).join('');
    const tracking = step.l !== undefined;
    const l = step.l ?? 0;
    const r = step.r ?? sample.length - 1;
    bars.forEach(({ bar, value }, barIndex) => {
      const edge = tracking && (barIndex === l || barIndex === r);
      const inside = tracking && barIndex > l && barIndex < r;
      bar.setAttribute('fill', edge ? '#99ffe4' : inside ? '#3a3a3a' : '#242424');
      value.style.fill = edge ? '#10231d' : '';
    });
    gsap.to([pointers.l, pointers.r], { opacity: tracking ? 1 : 0, duration: 0.3, overwrite: 'auto' });
    gsap.to(shown, {
      l, r, level: tracking ? Math.min(sample[l], sample[r]) * UNIT : 0,
      duration: reduced ? 0 : 0.5, ease: 'expo.out', overwrite: true, onUpdate: geometry,
    });
    const bestPair = bestPairs[index];
    gsap.to([best, bestLabel], { opacity: bestPair ? 1 : 0, duration: 0.3, overwrite: 'auto' });
    if (bestPair) {
      const level = Math.min(sample[bestPair[0]], sample[bestPair[1]]) * UNIT;
      gsap.set(best, { attr: { x: center(bestPair[0]), y: BASE - level, width: center(bestPair[1]) - center(bestPair[0]), height: level } });
      gsap.set(bestLabel, { attr: { x: center(bestPair[0]), y: BASE - level - 8 } });
      bestLabel.textContent = `best ${step.best}`;
    }
  };

  // Autoplay is one repeating delayed call; scrubbing and manual input pause it.
  let playing = !reduced;
  let visible = false;
  let scrubbing = false;
  const tick = gsap.delayedCall(STEP_SECONDS, function advance() {
    index = index === steps.length - 1 ? 0 : index + 1;
    render();
    tick.delay(index === steps.length - 1 ? STEP_SECONDS * 4 : STEP_SECONDS).restart(true);
  }).pause();
  const sync = () => {
    tick.paused(!(playing && visible && !scrubbing));
    card.classList.toggle('is-paused', !playing || scrubbing);
    play.setAttribute('aria-label', playing ? 'Pause' : 'Play');
  };
  const go = next => {
    const clamped = Math.max(0, Math.min(steps.length - 1, next));
    if (clamped === index) return;
    index = clamped;
    render();
  };

  play.addEventListener('click', () => { playing = !playing; sync(); });
  seek.addEventListener('input', () => { playing = false; sync(); go(Number(seek.value)); });
  card.addEventListener('keydown', event => {
    if (event.target === seek && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const moves = { ArrowRight: index + 1, ArrowLeft: index - 1, Home: 0, End: steps.length - 1 };
    if (event.key in moves) { event.preventDefault(); playing = false; sync(); go(moves[event.key]); }
    if (event.key === ' ') { event.preventDefault(); playing = !playing; sync(); }
  });
  geometry();
  render();
  sync();

  return {
    setVisible(value) { visible = value; sync(); },
    setScrubbing(value) { scrubbing = value; sync(); },
    scrub(progress) { go(Math.round(progress * (steps.length - 1))); },
  };
}
