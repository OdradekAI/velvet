export type {
  LayoutCommand,
  LayoutCommandId,
  LayoutOrderAnchor,
} from '../../platform/protocol.js';

import type { LayoutCommand } from '../../platform/protocol.js';

export interface LayoutCommandSink {
  dispatch(command: LayoutCommand): Promise<void> | void;
}
