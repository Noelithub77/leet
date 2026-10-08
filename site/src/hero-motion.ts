import { gsap } from 'gsap';

export function createHeroCycle(morph, letters, numbers, onMorph) {
  const hidden = { yPercent: 12, rotationX: -45, opacity: 0, filter: 'blur(2px)' };
  const shown = { yPercent: 0, rotationX: 0, opacity: 1, filter: 'blur(0px)' };
  gsap.set(letters, hidden);
  gsap.set(numbers, shown);
  gsap.set(morph, { value: 1, onUpdate: onMorph });
  const out = { yPercent: -12, rotationX: 45, opacity: 0, filter: 'blur(2px)', duration: 0.14, stagger: 0.008, ease: 'power2.in' };
  const into = { ...shown, duration: 0.24, stagger: 0.01, ease: 'power3.out', immediateRender: false };
  return gsap.timeline({ paused: true, repeat: -1 })
    .to(morph, { value: 0, duration: 1, ease: 'power2.inOut', onUpdate: onMorph }, 1.5)
    .to(numbers, out, 1.85)
    .fromTo(letters, hidden, into, 1.91)
    .to(morph, { value: 1, duration: 1, ease: 'power2.inOut', onUpdate: onMorph }, 4)
    .to(letters, out, 4.35)
    .fromTo(numbers, hidden, into, 4.41)
    .to({}, { duration: 1.5 }, 5);
}
