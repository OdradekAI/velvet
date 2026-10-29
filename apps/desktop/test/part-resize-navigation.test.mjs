import assert from 'node:assert/strict';
import { nextResizeSize } from '../src/app/renderer/part-resize-navigation.ts';

const horizontal = {
  axis: 'horizontal',
  currentSize: 260,
  min: 180,
  max: 520,
};

assert.equal(nextResizeSize('ArrowRight', horizontal), 270);
assert.equal(nextResizeSize('ArrowLeft', { ...horizontal, direction: -1 }), 270);
assert.equal(nextResizeSize('ArrowRight', { ...horizontal, large: true }), 300);
assert.equal(nextResizeSize('Home', horizontal), 180);
assert.equal(nextResizeSize('End', horizontal), 520);
assert.equal(nextResizeSize('PageUp', horizontal), undefined);
assert.equal(nextResizeSize('ArrowLeft', { ...horizontal, currentSize: 180 }), 180);

const vertical = {
  axis: 'vertical',
  currentSize: 180,
  min: 120,
  max: 520,
};

assert.equal(nextResizeSize('ArrowUp', vertical), 190);
assert.equal(nextResizeSize('ArrowDown', vertical), 170);
assert.equal(nextResizeSize('ArrowDown', { ...vertical, direction: -1 }), 190);
