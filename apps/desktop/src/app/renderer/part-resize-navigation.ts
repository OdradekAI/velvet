export type ResizeAxis = 'horizontal' | 'vertical';

export interface ResizeKeyboardOptions {
  readonly axis: ResizeAxis;
  /** Converts the logical ArrowRight/ArrowDown direction into a size delta. */
  readonly direction?: 1 | -1;
  readonly currentSize: number;
  readonly min: number;
  readonly max: number;
  readonly step?: number;
  readonly largeStep?: number;
  readonly large?: boolean;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}

/**
 * Resolves a keyboard resize gesture without knowing anything about React or
 * the layout transport. Home/End deliberately map to the Rust-owned bounds.
 */
export function nextResizeSize(
  key: string,
  options: ResizeKeyboardOptions,
): number | undefined {
  const min = Math.min(options.min, options.max);
  const max = Math.max(options.min, options.max);
  const current = clamp(options.currentSize, min, max);

  if (key === 'Home') {
    return min;
  }
  if (key === 'End') {
    return max;
  }

  const direction = options.direction ?? 1;
  const step = options.large
    ? options.largeStep ?? (options.step ?? 10) * 4
    : options.step ?? 10;
  let sign = 0;
  if (options.axis === 'horizontal') {
    if (key === 'ArrowRight') sign = 1;
    if (key === 'ArrowLeft') sign = -1;
  } else {
    if (key === 'ArrowDown') sign = -1;
    if (key === 'ArrowUp') sign = 1;
  }

  if (!sign) {
    return undefined;
  }

  return clamp(current + sign * direction * step, min, max);
}
