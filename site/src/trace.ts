// A deterministic replay of the Container With Most Water solution, step for step
// like leet's Python tracer: each step is a line event with the state before it runs.
export const source = [
  'class Solution:',
  '    def maxArea(self, height):',
  '        l, r = 0, len(height) - 1',
  '        best = 0',
  '        while l < r:',
  '            area = (r - l) * min(height[l], height[r])',
  '            best = max(best, area)',
  '            if height[l] < height[r]:',
  '                l += 1',
  '            else:',
  '                r -= 1',
  '        return best',
];

export const sample = [1, 8, 6, 2, 5, 4, 8, 3, 7];

export function maxAreaTrace(height) {
  const steps = [];
  const vars = {};
  const record = (line, event = 'line') => steps.push({ line, event, ...vars });
  record(2, 'call');
  record(3);
  vars.l = 0; vars.r = height.length - 1;
  record(4);
  vars.best = 0;
  while (true) {
    record(5);
    if (!(vars.l < vars.r)) break;
    record(6);
    vars.area = (vars.r - vars.l) * Math.min(height[vars.l], height[vars.r]);
    record(7);
    vars.best = Math.max(vars.best, vars.area);
    record(8);
    if (height[vars.l] < height[vars.r]) { record(9); vars.l += 1; }
    else { record(11); vars.r -= 1; }
  }
  record(12);
  record(12, 'return');
  return steps;
}

/** How often each line has run up to and including `index`, for the heat gutter. */
export function lineHeat(steps, index) {
  const heat = new Array(source.length + 1).fill(0);
  for (const step of steps.slice(0, index + 1)) if (step.event === 'line') heat[step.line] += 1;
  return heat;
}
