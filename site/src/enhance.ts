// Motion layer, loaded after the hero is interactive. GSAP drives every timeline; loops pause offscreen.
import { gsap } from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';
import { SplitText } from 'gsap/SplitText';
import { DrawSVGPlugin } from 'gsap/DrawSVGPlugin';
import { ScrambleTextPlugin } from 'gsap/ScrambleTextPlugin';
import Lenis from 'lenis';
import 'lenis/dist/lenis.css';
import { mountDebugger } from './debugger';
import { colors, pointsAttribute, strokeAt } from './logo';
import { snapRefreshRate } from './refresh';
import { createHeroCycle } from './hero-motion';

gsap.registerPlugin(ScrollTrigger, SplitText, DrawSVGPlugin, ScrambleTextPlugin);

const root = document.documentElement;
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const saveData = Boolean(navigator.connection?.saveData);
const base = import.meta.env.BASE_URL;
const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
const whileVisible = (trigger, onToggle, start = 'top bottom', end = 'bottom top') =>
  ScrollTrigger.create({ trigger, start, end, onToggle: self => onToggle(self.isActive) });

/* Smooth scrolling, synced with ScrollTrigger on GSAP's ticker */
if (!reduced) {
  const lenis = new Lenis({ autoRaf: false, lerp: 0.12, anchors: { offset: -72 } });
  lenis.on('scroll', ScrollTrigger.update);
  gsap.ticker.add(time => lenis.raf(time * 1000));
  gsap.ticker.lagSmoothing(0);
}

/* Header and scroll cue */
ScrollTrigger.create({
  start: 40,
  end: 'max',
  onToggle: self => {
    $('.bar').classList.toggle('is-scrolled', self.isActive);
    root.classList.toggle('is-scrolled-past', self.isActive);
  },
});

/* Scroll progress: the page as a recording */
gsap.to('.bar-progress', { scaleX: 1, ease: 'none', scrollTrigger: { start: 0, end: 'max', scrub: 0.3 } });

/* Reveals */
if (!reduced) {
  gsap.set('[data-reveal]', { opacity: 0, y: 32 });
  ScrollTrigger.batch('[data-reveal]', {
    start: 'top 90%',
    once: true,
    onEnter: batch => gsap.to(batch, { opacity: 1, y: 0, duration: 1.1, ease: 'expo.out', stagger: 0.08, overwrite: true }),
  });
}

/* Logo ⇄ 1337 morphs */
const marks = $$('[data-mark]').map(svg => {
  const initial = svg.closest('.hero') ? 1 : 0;
  const lines = [...svg.querySelectorAll('polyline')];
  const strokes = lines.map(() => ({ p: initial }));
  const draw = () => strokes.forEach(({ p }, index) => {
    const stroke = strokeAt(index, p);
    lines[index].setAttribute('points', pointsAttribute(stroke.points));
    lines[index].setAttribute('stroke-width', stroke.width.toFixed(2));
  });
  const morph = { duration: reduced ? 0 : 0.75, ease: 'power2.inOut', stagger: 0.045, onUpdate: draw };
  draw();
  const loop = gsap.timeline({ paused: true, repeat: -1, yoyo: true, repeatDelay: 1.5, delay: 1.2 }).to(strokes, { p: 1 - initial, ...morph });
  const mark = { svg, hold: false, visible: false, ghosted: false };
  mark.setMorph = value => { strokes.forEach(stroke => { stroke.p = value; }); draw(); };
  mark.to = target => { loop.pause(); return gsap.to(strokes, { p: target, ...morph, overwrite: true }); };
  mark.resume = () => { if (!mark.hold && !mark.ghosted && mark.visible && !reduced) mark.to(initial).then(() => !mark.hold && loop.restart(true)); };
  // Particles take over the hero mark; the SVG keeps its layout and accessible name.
  mark.ghost = () => { mark.ghosted = true; loop.pause(); svg.classList.add('is-ghost'); };
  whileVisible(svg, visible => {
    mark.visible = visible;
    if (visible && !svg.closest('.hero') && !mark.hold && !mark.ghosted && !reduced) loop.play();
    else loop.pause();
  });
  if (!svg.closest('.hero')) {
    svg.addEventListener('pointerenter', () => !mark.ghosted && mark.to(1));
    svg.addEventListener('pointerleave', () => mark.resume());
  }
  return mark;
});

/* Hero shader with an eased pointer light, in its own chunk */
const hero = $('.hero');
const canvas = $('[data-shader]');
const shaderReady = !reduced && !saveData && canvas ? import('./shader').then(({ mountShader }) => {
  const shader = mountShader(canvas);
  if (!shader) return null;
  const render = time => shader.render(time);
  whileVisible(hero, visible => (visible ? gsap.ticker.add(render) : gsap.ticker.remove(render)));
  const x = gsap.quickTo(shader.pointer, 'x', { duration: 0.25, ease: 'power2.out' });
  const y = gsap.quickTo(shader.pointer, 'y', { duration: 0.25, ease: 'power2.out' });
  hero.addEventListener('pointermove', event => {
    const { width, height } = hero.getBoundingClientRect();
    x((event.clientX / width - 0.5) * (width / height));
    y(0.5 - event.clientY / height);
  }, { passive: true });
  return shader;
}) : Promise.resolve(null);

/* GPU particle mark: flows logo ⇄ 1337, dodges the pointer, scatters on scroll */
const particlesReady = !reduced && !saveData && marks[0] ? import('./particles').then(({ mountParticles }) => {
  const canvas = $('[data-particles]');
  const particles = mountParticles(canvas, marks[0].svg);
  if (!particles) { root.classList.remove('hero-pending'); return null; }
  marks[0].ghost();
  root.classList.remove('hero-pending');
  canvas.classList.add('is-live');
  const render = time => particles.render(time);
  whileVisible(hero, visible => visible ? gsap.ticker.add(render) : gsap.ticker.remove(render));
  const x = gsap.quickTo(particles.pointer, 'x', { duration: 0.12, ease: 'power2.out' });
  const y = gsap.quickTo(particles.pointer, 'y', { duration: 0.12, ease: 'power2.out' });
  hero.addEventListener('pointermove', event => {
    const box = canvas.getBoundingClientRect();
    x(event.clientX - box.left);
    y(event.clientY - box.top);
  }, { passive: true });
  hero.addEventListener('pointerleave', () => { x(-800); y(-800); });
  document.fonts?.ready.then(particles.layout);
  return particles;
}).catch(() => { root.classList.remove('hero-pending'); return null; }) : Promise.resolve(null);

/* One clock keeps the particle/SVG morph and letter flip paired, including after pauses. */
const heroMotionReady = particlesReady.then(particles => {
  const heroWord = $('[data-hero-word]');
  if (!heroWord || reduced || saveData) return null;
  const leet = heroWord.querySelector('[data-spelling="leet"]');
  const digits = heroWord.querySelector('[data-spelling="1337"]');
  const letters = SplitText.create(leet, { type: 'chars', charsClass: 'spelling-char' });
  const numbers = SplitText.create(digits, { type: 'chars', charsClass: 'spelling-char' });
  const morph = particles?.uniforms.uMorph ?? { value: 1 };
  const cycle = createHeroCycle(morph, letters.chars, numbers.chars,
    particles ? undefined : () => marks[0].setMorph(morph.value));
  gsap.set([leet, digits], { opacity: 1, y: 0 });
  let visible = false;
  const hold = on => {
    cycle.pause();
    if (on) cycle.totalTime(0);
    else { cycle.totalTime(0); if (visible) cycle.play(); }
  };
  whileVisible(hero, on => {
    visible = on;
    if (on && !root.classList.contains('eleet')) cycle.play();
    else cycle.pause();
  });
  if (root.classList.contains('eleet')) hold(true);
  return { hold };
});

/* Scroll scenes, per viewport */
const debuggerCard = $('[data-debugger]');
const replay = debuggerCard ? mountDebugger(debuggerCard, { reduced }) : null;
if (replay) whileVisible(debuggerCard, visible => replay.setVisible(visible), 'top 85%', 'bottom 15%');

const media = gsap.matchMedia();
media.add({ desktop: '(min-width: 900px)', motion: '(prefers-reduced-motion: no-preference)' }, context => {
  const { desktop, motion } = context.conditions;
  if (!motion) return;

  // The hero recedes while the app window rises and flattens.
  const title = SplitText.create('[data-split] .hz', { type: 'chars', charsClass: 'char' });
  const reveal = gsap.timeline({ scrollTrigger: { trigger: hero, start: 'top top', end: 'bottom 20%', scrub: true } })
    .fromTo('[data-stage-window]', { rotateX: desktop ? 26 : 12, y: desktop ? 60 : 30, scale: desktop ? 0.8 : 0.92, transformPerspective: 1600, transformOrigin: '50% 0%' }, { rotateX: 0, y: 0, scale: 1, ease: 'none' }, 0)
    .to('[data-hero]', { y: desktop ? -140 : -60, scale: 0.94, opacity: 0, ease: 'none' }, 0)
    .to(title.chars, { yPercent: -60, opacity: 0, stagger: { each: 0.02, from: 'center' }, ease: 'none' }, 0);
  void shaderReady.then(shader => shader && reveal.to(shader.uniforms.uIntensity, { value: 0.2, ease: 'none' }, 0));
  void particlesReady.then(particles => particles && reveal.to(particles.uniforms.uScatter, { value: 1, ease: 'power2.in' }, 0));

  // Headings rise line by line out of a mask; section numbers scramble through 1337.
  const headings = $$('.section-head h2, .get h2').map(heading => SplitText.create(heading, { type: 'lines', mask: 'lines', linesClass: 'line' }));
  for (const split of headings) {
    gsap.from(split.lines, { yPercent: 110, duration: 1.2, ease: 'expo.out', stagger: 0.1, scrollTrigger: { trigger: split.elements[0], start: 'top 88%', once: true } });
  }
  for (const number of $$('.kicker span')) {
    ScrollTrigger.create({ trigger: number, start: 'top 90%', once: true, onEnter: () => gsap.to(number, { duration: 0.9, scrambleText: { text: number.textContent, chars: '1337', speed: 0.6 } }) });
  }

  gsap.to('.marquee-track', { xPercent: -36, ease: 'none', scrollTrigger: { trigger: '.marquee', start: 'top bottom', end: 'bottom top', scrub: 0.6 } });

  if (desktop && replay) {
    // Scrolling scrubs the recording while the card is pinned; autoplay resumes after.
    ScrollTrigger.create({
      trigger: debuggerCard,
      start: 'center 54%',
      end: '+=140%',
      pin: true,
      scrub: true,
      onUpdate: self => replay.scrub(self.progress),
      onToggle: self => { replay.setScrubbing(self.isActive); $('[data-scrub-hint]').classList.toggle('is-active', self.isActive); },
    });
  }

  if (desktop) {
    // Horizontal gallery driven by vertical scroll.
    const track = $('[data-gallery-track]');
    const distance = () => track.scrollWidth - root.clientWidth;
    $('[data-gallery]').classList.add('is-pinned');
    const pan = gsap.to(track, {
      x: () => -distance(),
      ease: 'none',
      scrollTrigger: { trigger: '#tour', start: 'top top', end: () => `+=${distance()}`, pin: true, scrub: 0.8, invalidateOnRefresh: true },
    });
    for (const figure of track.children) {
      gsap.fromTo(figure, { scale: 0.88, opacity: 0.45 }, { scale: 1, opacity: 1, ease: 'none', scrollTrigger: { trigger: figure, containerAnimation: pan, start: 'left right', end: 'center 55%', scrub: true } });
    }
    return () => { $('[data-gallery]').classList.remove('is-pinned'); title.revert(); headings.forEach(split => split.revert()); };
  }
  return () => { title.revert(); headings.forEach(split => split.revert()); };
});

/* Live refresh-rate meter on GSAP's ticker (it runs once per display frame) */
const meter = $('[data-meter]');
if (meter && reduced) meter.querySelector('.meter-label').textContent = 'Built for high-refresh displays.';
if (meter && !reduced) {
  meter.querySelector('.meter-label').textContent = 'Browser frame cadence, measured live. Each cell is one frame.';
  const readout = meter.querySelector('[data-hz]');
  const grid = meter.querySelector('[data-frames]');
  let cells = [];
  const build = count => {
    grid.replaceChildren(...Array.from({ length: count }, () => document.createElement('i')));
    cells = [...grid.children];
  };
  build(120);
  // 30 fps, 60 fps, and native lanes: the same motion sampled at different rates.
  const lanes = $$('[data-lanes] .lane').map(lane => ({ fps: Number(lane.dataset.fps), dot: lane.querySelector('i'), track: lane.querySelector('b') }));
  const nativeLabel = meter.querySelector('[data-native-label]');
  let frame = 0;
  const deltas = [];
  const tick = (_time, delta) => {
    if (deltas.length < 90) {
      deltas.push(delta);
      const measured = deltas.length === 90 ? snapRefreshRate(deltas) : null;
      if (measured && measured !== cells.length) { readout.textContent = String(measured); build(Math.min(measured, 240)); }
      if (measured) nativeLabel.textContent = `leet · ${measured} Hz`;
    }
    for (const { fps, dot, track } of lanes) {
      const t = fps ? Math.floor(_time * fps) / fps : _time;
      const progress = 0.5 - Math.cos(((t % 1.6) / 1.6) * Math.PI * 2) / 2;
      dot.style.transform = `translateX(${progress * (track.clientWidth - 20)}px)`;
    }
    cells[(frame - 1 + cells.length) % cells.length]?.classList.remove('on');
    cells[frame % cells.length]?.classList.add('on');
    frame += 1;
  };
  whileVisible(meter, visible => {
    if (visible) gsap.ticker.add(tick);
    else gsap.ticker.remove(tick);
  });
}

/* Videos attach near the viewport and play only while visible */
const highRefresh = new Promise(resolve => {
  const deltas = [];
  const sample = (_time, delta) => {
    deltas.push(delta);
    if (deltas.length < 30) return;
    gsap.ticker.remove(sample);
    resolve((snapRefreshRate(deltas.slice(2)) ?? 60) >= 100);
  };
  gsap.ticker.add(sample);
});
const av1 = document.createElement('video').canPlayType('video/webm; codecs="av01.0.08M.08"') !== '';
if (!reduced && !saveData) {
  for (const frame of $$('[data-media][data-video]')) {
    let video = null;
    let visible = false;
    ScrollTrigger.create({
      trigger: frame,
      start: 'top bottom+=600',
      once: true,
      onEnter: async () => {
        video = document.createElement('video');
        Object.assign(video, { muted: true, loop: true, playsInline: true, preload: 'auto', disablePictureInPicture: true });
        video.setAttribute('aria-hidden', 'true');
        const name = frame.dataset.media;
        const sources = [];
        if ('hfr' in frame.dataset && av1 && innerWidth >= 700 && await highRefresh) sources.push([`${name}-120.webm`, 'video/webm; codecs="av01.0.08M.08"']);
        if (av1) sources.push([`${name}.webm`, 'video/webm; codecs="av01.0.08M.08"']);
        sources.push([`${name}.mp4`, 'video/mp4']);
        for (const [file, type] of sources) video.append(Object.assign(document.createElement('source'), { src: `${base}media/${file}`, type }));
        video.addEventListener('playing', () => video.classList.add('is-ready'), { once: true });
        frame.append(video);
        if (visible) void video.play().catch(() => {});
      },
    });
    whileVisible(frame, active => {
      visible = active;
      if (video) active ? void video.play().catch(() => {}) : video.pause();
    }, 'top 80%', 'bottom 20%');
  }
}

/* Assist tiles light up in turn */
const tiles = $$('[data-actions] li');
if (tiles.length && !reduced) {
  const cycle = gsap.timeline({ repeat: -1, paused: true });
  tiles.forEach((tile, index) => cycle.call(() => tiles.forEach(other => other.classList.toggle('is-lit', other === tile)), null, index * 0.85));
  cycle.to({}, { duration: 0.85 });
  whileVisible('[data-actions]', visible => (visible ? cycle.play() : cycle.pause()));
}

/* Competitive Companion: press, beam, samples land */
const companion = $('[data-cph]');
if (companion && !reduced) {
  const cases = companion.querySelectorAll('.cph-case');
  const send = gsap.timeline({ repeat: -1, repeatDelay: 0.6, paused: true })
    .set(cases, { opacity: 0, x: -16 })
    .fromTo('[data-beam]', { drawSVG: '0% 0%' }, { drawSVG: '0% 0%', duration: 0.5 })
    .to('.cph-button', { scale: 0.92, duration: 0.12, ease: 'power2.in' })
    .to('.cph-button', { scale: 1, duration: 0.5, ease: 'elastic.out(1, .5)' })
    .to('[data-beam]', { drawSVG: '0% 35%', duration: 0.35, ease: 'power2.in' }, '<')
    .to('[data-beam]', { drawSVG: '100% 100%', duration: 0.55, ease: 'power2.out' })
    .to(cases, { opacity: 1, x: 0, duration: 0.6, ease: 'expo.out', stagger: 0.1 }, '-=0.15')
    .to(cases, { opacity: 0, duration: 0.4, delay: 1.6 });
  whileVisible(companion, visible => (visible ? send.play() : send.pause()));
}

/* Typed Rust */
const code = $('[data-type] code');
if (code && !reduced) {
  const nodes = [];
  const walker = document.createTreeWalker(code, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) nodes.push([walker.currentNode, walker.currentNode.textContent]);
  const total = nodes.reduce((sum, [, text]) => sum + text.length, 0);
  const typed = { count: 0 };
  const paint = () => {
    let left = Math.round(typed.count);
    for (const [node, text] of nodes) { node.textContent = text.slice(0, Math.max(0, left)); left -= text.length; }
  };
  paint();
  ScrollTrigger.create({
    trigger: code,
    start: 'top 75%',
    once: true,
    onEnter: () => {
      code.parentElement.classList.add('is-typing');
      gsap.to(typed, { count: total, duration: 1.8, ease: 'none', onUpdate: paint, onComplete: () => gsap.delayedCall(1.4, () => code.parentElement.classList.remove('is-typing')) });
    },
  });
}

/* Counters: 0 unsafe counts down from 1337 */
for (const number of $$('[data-count]')) {
  const target = Number(number.dataset.count);
  const value = { n: target === 0 ? 1337 : 0 };
  if (reduced) continue;
  number.textContent = String(value.n);
  ScrollTrigger.create({
    trigger: number,
    start: 'top 85%',
    once: true,
    onEnter: () => gsap.to(value, { n: target, duration: 1.6, ease: 'expo.out', snap: { n: 1 }, onUpdate: () => { number.textContent = String(value.n); } }),
  });
}

/* Type 1337 for eleet mode */
let typedKeys = '';
document.addEventListener('keydown', event => {
  if (event.ctrlKey || event.metaKey || event.altKey || event.target instanceof HTMLInputElement) return;
  typedKeys = (typedKeys + event.key).slice(-4);
  if (typedKeys !== '1337') return;
  typedKeys = '';
  const on = root.classList.toggle('eleet');
  for (const mark of marks.filter(mark => !mark.svg.closest('.hero'))) { mark.hold = on; on ? mark.to(1) : mark.resume(); }
  void heroMotionReady.then(motion => motion?.hold(on));
  if (!reduced) {
    const glyphs = Array.from({ length: 28 }, (_, index) => {
      const glyph = Object.assign(document.createElement('span'), { className: 'glyph', textContent: '1337'[index % 4] });
      glyph.style.color = colors[index % 4];
      document.body.append(glyph);
      return glyph;
    });
    gsap.fromTo(glyphs,
      { x: () => gsap.utils.random(0, innerWidth), y: -40, rotation: 0, opacity: 1 },
      { y: () => innerHeight + 60, rotation: () => gsap.utils.random(-300, 300), opacity: 0.2, duration: () => gsap.utils.random(1.1, 2), ease: 'power1.in', stagger: 0.02, onComplete: () => glyphs.forEach(glyph => glyph.remove()) });
  }
  void import('./toast').then(toast => toast.eleet(on));
});

/* Soft spotlight follows the pointer across cards */
document.addEventListener('pointermove', event => {
  const card = event.target instanceof Element ? event.target.closest('.trio article, .providers li, .stats div, .meter, .install, .cph-card') : null;
  if (!card) return;
  const box = card.getBoundingClientRect();
  card.style.setProperty('--mx', `${event.clientX - box.left}px`);
  card.style.setProperty('--my', `${event.clientY - box.top}px`);
}, { passive: true });

/* Layout settles after fonts and images */
document.fonts?.ready.then(() => ScrollTrigger.refresh());
addEventListener('load', () => ScrollTrigger.refresh(), { once: true });
