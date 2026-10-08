// The logo reads 1337: a peach dot, two mint chevrons, and a peach bracket.
// Each stroke has five points in both forms so they can morph point by point.
const dot = [13.1, 32];

export const logo = [
  { points: [dot, dot, dot, dot, [dot[0], dot[1] + 0.01]], width: 6.4 },
  { points: [[17.6, 21], [23.1, 26.5], [28.6, 32], [23.1, 37.5], [17.6, 43]], width: 5 },
  { points: [[29.1, 21], [34.6, 26.5], [40.1, 32], [34.6, 37.5], [29.1, 43]], width: 5 },
  { points: [[25.1, 5.5], [38.35, 18.75], [51.6, 32], [38.35, 45.25], [25.1, 58.5]], width: 5 },
];

const three = x => [[x, 18], [x + 9, 18], [x + 3, 29.5], [x + 9.5, 37], [x + 1, 46]];

export const digits = [
  { points: [[8, 23.5], [10.5, 20.75], [13, 18], [13, 32], [13, 46]], width: 4.6 },
  { points: three(18.5), width: 4.6 },
  { points: three(31.5), width: 4.6 },
  { points: [[45.5, 18], [50.5, 18], [55.5, 18], [51.75, 32], [48, 46]], width: 4.6 },
];

export const colors = ['#ffc799', '#99ffe4', '#99ffe4', '#ffc799'];

/** Stroke `index` between the logo (p = 0) and its 1337 digit (p = 1); GSAP supplies easing. */
export function strokeAt(index, p) {
  const from = logo[index];
  const to = digits[index];
  return {
    points: from.points.map(([x, y], point) => [x + (to.points[point][0] - x) * p, y + (to.points[point][1] - y) * p]),
    width: from.width + (to.width - from.width) * p,
  };
}

export const pointsAttribute = points => points.map(([x, y]) => `${x.toFixed(2)},${y.toFixed(2)}`).join(' ');
