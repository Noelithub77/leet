import { Mesh, Program, Renderer, Triangle } from 'ogl';

const vertex = /* glsl */ `
attribute vec2 position;
attribute vec2 uv;
varying vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position, 0.0, 1.0); }
`;

// Vesper black with slow mint and peach light and a responsive pointer glow.
const fragment = /* glsl */ `
precision highp float;
uniform float uTime;
uniform vec2 uResolution;
uniform vec2 uPointer;
uniform float uIntensity;
varying vec2 vUv;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) {
  float value = 0.0, amplitude = 0.5;
  for (int i = 0; i < 4; i++) { value += amplitude * noise(p); p *= 2.03; amplitude *= 0.5; }
  return value;
}

void main() {
  vec2 p = (vUv - 0.5) * vec2(uResolution.x / uResolution.y, 1.0);
  float t = uTime * 0.045;
  float warp = fbm(p * 1.4 + vec2(t, -t * 0.7));
  float field = fbm(p * 2.1 + warp * 1.6 - vec2(t * 0.6, t * 0.25));
  vec3 base = vec3(0.063);
  vec3 mint = vec3(0.6, 1.0, 0.894);
  vec3 peach = vec3(1.0, 0.78, 0.6);
  float mintLight = smoothstep(0.42, 0.95, field) * smoothstep(1.1, 0.1, length(p - vec2(-0.25, 0.25)));
  float peachLight = smoothstep(0.5, 1.0, warp) * smoothstep(1.2, 0.2, length(p - vec2(0.55, -0.35)));
  vec3 color = base + mint * mintLight * 0.16 + peach * peachLight * 0.11;
  color += mint * 0.07 * exp(-5.0 * length(p - uPointer));
  color *= 1.0 - 0.6 * dot(p * 0.85, p * 0.85);
  gl_FragColor = vec4(mix(base, color, uIntensity), 1.0);
}
`;

/** Starts the hero shader; returns controls, or null when WebGL is unavailable. */
export function mountShader(canvas) {
  let renderer;
  try {
    // Half resolution keeps the soft field cheap on phones; CSS scales it up.
    renderer = new Renderer({ canvas, dpr: Math.min(devicePixelRatio, 2) * 0.5, alpha: false, antialias: false, powerPreference: 'low-power' });
  } catch {
    return null;
  }
  const { gl } = renderer;
  if (!gl) return null;
  const program = new Program(gl, {
    vertex,
    fragment,
    uniforms: {
      uTime: { value: 0 },
      uResolution: { value: [1, 1] },
      uPointer: { value: [0, 0] },
      uIntensity: { value: 1 },
    },
  });
  const mesh = new Mesh(gl, { geometry: new Triangle(gl), program });
  const resize = () => {
    const { width, height } = canvas.parentElement.getBoundingClientRect();
    renderer.setSize(width, height);
    program.uniforms.uResolution.value = [width, height];
  };
  resize();
  addEventListener('resize', resize, { passive: true });
  const pointer = { x: 0, y: 0 };
  const render = time => {
    program.uniforms.uTime.value = time;
    program.uniforms.uPointer.value = [pointer.x, pointer.y];
    renderer.render({ scene: mesh });
  };
  render(12);
  canvas.classList.add('is-live');
  return { pointer, uniforms: program.uniforms, render };
}
