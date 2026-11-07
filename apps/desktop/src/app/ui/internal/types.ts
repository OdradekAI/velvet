export const UI_KIT_API_VERSION = '1.0' as const;

export type UiButtonVariant = 'default' | 'quiet' | 'danger';

export interface UiCommonProps {
  readonly [key: `data-${string}`]: string | boolean | undefined;
  readonly apiVersion?: string;
  readonly testId?: string;
  readonly ariaLabel?: string;
  readonly describedBy?: string;
  readonly disabled?: boolean;
  readonly busy?: boolean;
  readonly ariaLive?: 'off' | 'polite' | 'assertive';
}
