import { UI_KIT_API_VERSION, type UiCommonProps } from './types.js';

export function uiDataProps(props: UiCommonProps): Record<string, string | undefined> {
  const dataProps: Record<string, string | undefined> = {
    'data-ui-kit-api-version': props.apiVersion ?? UI_KIT_API_VERSION,
    'data-ui-test-id': props.testId,
    'aria-label': props.ariaLabel,
    'aria-describedby': props.describedBy,
    'aria-busy': props.busy ? 'true' : undefined,
    'aria-live': props.ariaLive,
  };
  for (const [key, value] of Object.entries(props)) {
    if (key.startsWith('data-')) {
      dataProps[key] = typeof value === 'boolean' ? String(value) : value;
    }
  }
  return dataProps;
}
