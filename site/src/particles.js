import { Geometry, Mesh, Program, Renderer } from 'ogl';
import { colors, digits, logo } from './logo.js';

// Each stroke is sampled as dots and paired by position along the stroke,
// so morphing makes particles flow along their own path (1 → dot, 3 → chevron, 7 → bracket).
const SAMPLE = 160;

function along(points, [x, y]) {
  let best = { distance: Infinity, t: 0 };
  const lengths = points.slice(1).map((point, index) => Math.hypot(point[0] - points[index][0], point[1] - points[index][1]));
  const total = lengths.reduce((sum, length) => sum + length, 0) || 1;
  let walked = 0;
  points.slice(1).forEach(([bx, by], index) => {
    const [ax, ay] = points[index];
    const dx = bx - ax;
    const dy = by - ay;
    const span = dx * dx + dy * dy || 1;
    const u = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / span));
    const distance = Math.hypot(x - (ax + dx * u), y - (ay + dy * u));
    if (distance < best.distance) best = { distance, t: (walked + lengths[index] * u) / total };
    walked += lengths[index];
  });
  return best.t;
}

function sampleStroke(stroke) {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = SAMPLE;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  context.scale(SAMPLE / 64, SAMPLE / 64);
  context.lineCap = context.lineJoin = 'round';
  context.lineWidth = stroke.width;
  context.beginPath();
  stroke.points.forEach(([x, y], index) => (index ? context.lineTo(x, y) : context.moveTo(x, y)));
  context.stroke();
  const { data } = context.getImageData(0, 0, SAMPLE, SAMPLE);
  const dots = [];
  for (let y = 0; y < SAMPLE; y += 1) {
    for (let x = (y % 2); x < SAMPLE; x += 2) {
      if (data[(y * SAMPLE + x) * 4 + 3] < 140) continue;
      const point = [(x / SAMPLE) * 64, (y / SAMPLE) * 64];
      dots.push({ point, t: along(stroke.points, point) });
    }
  }
  return dots.sort((a, b) => a.t - b.t);
}

function buildAttributes() {
  const from = [];
  const to = [];
  const color = [];
  const seed = [];
  const weight = [];
  logo.forEach((stroke, index) => {
    const a = sampleStroke(stroke);
    const b = sampleStroke(digits[index]);
    const count = Math.max(a.length, b.length);
    const rgb = colors[index].match(/\w\w/g).map(hex => parseInt(hex, 16) / 255);
    for (let k = 0; k < count; k += 1) {
      from.push(...a[Math.floor((k / count) * a.length)].point);
      to.push(...b[Math.floor((k / count) * b.length)].point);
      color.push(...rgb);
      seed.push(Math.random());
      // Fewer samples means more particles share a spot; dim them so overlaps stay in color.
      weight.push(Math.sqrt(a.length / count), Math.sqrt(b.length / count));
    }
  });
  return { from: new Float32Array(from), to: new Float32Array(to), color: new Float32Array(color), seed: new Float32Array(seed), weight: new Float32Array(weight) };
}

const vertex = /* glsl */ `
attribute vec2 aFrom;
attribute vec2 aTo;
attribute vec3 aColor;
attribute float aSeed;
attribute vec2 aWeight;
uniform float uMorph;
uniform float uTime;
uniform float uScatter;
uniform float uSize;
uniform float uDpr;
uniform vec2 uCenter;
uniform vec2 uResolution;
uniform vec2 uPointer;
varying vec3 vColor;
varying float vAlpha;

void main() {
  float delay = aSeed * 0.3;
  float m = smoothstep(delay, delay + 0.7, uMorph);
  float swirl = sin(m * 3.14159);
  vec2 p = (mix(aFrom, aTo, m) - 32.0) / 64.0 * uSize + uCenter;
  float angle = aSeed * 6.2831 + uTime * (0.5 + aSeed);
  p += vec2(cos(angle), sin(angle)) * (0.35 + swirl * 8.0 * aSeed);
  vec2 away = p - uPointer;
  float near = exp(-dot(away, away) / 1100.0);
  p += normalize(away + 0.0001) * 20.0 * near;
  vec2 direction = vec2(cos(aSeed * 91.7), sin(aSeed * 53.3));
  p += direction * uScatter * (220.0 + 700.0 * fract(aSeed * 13.1));
  vec2 clip = p / uResolution * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  gl_PointSize = (2.1 + swirl * 1.4 + near * 1.6) * uDpr * clamp(uSize / 170.0, 1.0, 1.65);
  vColor = mix(aColor, vec3(1.0), near * 0.5 + swirl * 0.25);
  vAlpha = (0.7 + 0.3 * sin(uTime * 3.0 + aSeed * 40.0)) * (1.0 - uScatter * 0.85) * mix(aWeight.x, aWeight.y, m);
}
`;

const fragment = /* glsl */ `
precision highp float;
varying vec3 vColor;
varying float vAlpha;
void main() {
  float edge = smoothstep(0.5, 0.15, length(gl_PointCoord - 0.5));
  gl_FragColor = vec4(vColor, edge * vAlpha);
}
`;

/** GPU particle version of the mark, positioned over `anchor`. Null without WebGL. */
export function mountParticles(canvas, anchor) {
  let renderer;
  try {
    renderer = new Renderer({ canvas, dpr: Math.min(devicePixelRatio, 2), alpha: true, premultipliedAlpha: false, antialias: false });
  } catch {
    return null;
  }
  const { gl } = renderer;
  if (!gl) return null;
  gl.clearColor(0, 0, 0, 0);
  const attributes = buildAttributes();
  const geometry = new Geometry(gl, {
    aFrom: { size: 2, data: attributes.from },
    aTo: { size: 2, data: attributes.to },
    aColor: { size: 3, data: attributes.color },
    aSeed: { size: 1, data: attributes.seed },
    aWeight: { size: 2, data: attributes.weight },
  });
  const uniforms = {
    uMorph: { value: 1 }, uTime: { value: 0 }, uScatter: { value: 0 }, uSize: { value: 120 }, uDpr: { value: renderer.dpr },
    uCenter: { value: [0, 0] }, uResolution: { value: [1, 1] }, uPointer: { value: [-9999, -9999] },
  };
  const program = new Program(gl, { vertex, fragment, uniforms, transparent: true, depthTest: false, depthWrite: false });
  // Additive blending makes overlapping particles glow against Vesper black.
  program.setBlendFunc(gl.SRC_ALPHA, gl.ONE);
  const mesh = new Mesh(gl, { mode: gl.POINTS, geometry, program });
  const layout = () => {
    // OGL writes an inline size onto the canvas, so measure its container.
    const box = canvas.parentElement.getBoundingClientRect();
    const mark = anchor.getBoundingClientRect();
    renderer.setSize(box.width, box.height);
    uniforms.uResolution.value = [box.width, box.height];
    uniforms.uCenter.value = [mark.left - box.left + mark.width / 2, mark.top - box.top + mark.height / 2];
    uniforms.uSize.value = mark.width;
  };
  layout();
  new ResizeObserver(layout).observe(canvas.parentElement);
  const pointer = { x: -9999, y: -9999 };
  const render = time => {
    uniforms.uTime.value = time;
    uniforms.uPointer.value = [pointer.x, pointer.y];
    renderer.render({ scene: mesh });
  };
  render(0);
  return { uniforms, pointer, render, layout, count: attributes.seed.length };
}
