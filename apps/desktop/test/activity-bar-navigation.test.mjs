import assert from 'node:assert/strict';
import { nextActivityBarIndex } from '../src/app/renderer/activity-bar-navigation.ts';

assert.equal(nextActivityBarIndex('ArrowDown', 0, 3), 1);
assert.equal(nextActivityBarIndex('ArrowRight', 2, 3), 0);
assert.equal(nextActivityBarIndex('ArrowUp', 0, 3), 2);
assert.equal(nextActivityBarIndex('ArrowLeft', 1, 3), 0);
assert.equal(nextActivityBarIndex('Home', 2, 3), 0);
assert.equal(nextActivityBarIndex('End', 0, 3), 2);
assert.equal(nextActivityBarIndex('Enter', 1, 3), undefined);
assert.equal(nextActivityBarIndex('ArrowDown', 0, 0), undefined);

console.log('Activity Bar keyboard navigation tests passed.');
