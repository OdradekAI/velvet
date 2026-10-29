export type ActivityBarNavigationKey =
  | 'ArrowDown'
  | 'ArrowRight'
  | 'ArrowUp'
  | 'ArrowLeft'
  | 'Home'
  | 'End';

/** Returns the roving-focus target for an Activity Bar key press. */
export function nextActivityBarIndex(
  key: string,
  index: number,
  count: number,
): number | undefined {
  if (count <= 0 || index < 0 || index >= count) {
    return undefined;
  }
  if (key === 'Home') {
    return 0;
  }
  if (key === 'End') {
    return count - 1;
  }
  if (key === 'ArrowDown' || key === 'ArrowRight') {
    return (index + 1) % count;
  }
  if (key === 'ArrowUp' || key === 'ArrowLeft') {
    return (index - 1 + count) % count;
  }
  return undefined;
}
