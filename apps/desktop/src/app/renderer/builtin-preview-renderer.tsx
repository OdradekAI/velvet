import { useEffect, useState } from 'react';
import type { CorePreviewSession } from '../../platform/protocol.js';

export interface BuiltinPreviewRendererProps {
  readonly renderer: CorePreviewSession['renderer'];
}

/**
 * Host-owned renderers for trusted built-in providers. The renderer receives
 * only the typed projection returned by Rust; it never resolves an asset id or
 * reads a source location from the browser.
 */
export function BuiltinPreviewRenderer({ renderer }: BuiltinPreviewRendererProps) {
  switch (renderer.kind) {
    case 'text':
      return (
        <pre data-preview-text data-preview-truncated={renderer.truncated || undefined}>
          {renderer.text}
        </pre>
      );
    case 'image':
      return <BuiltinImagePreviewRenderer renderer={renderer} />;
    default:
      return null;
  }
}

function BuiltinImagePreviewRenderer({
  renderer,
}: {
  readonly renderer: Extract<CorePreviewSession['renderer'], { readonly kind: 'image' }>;
}) {
  const [src, setSrc] = useState<string>();

  useEffect(() => {
    if (typeof URL.createObjectURL !== 'function') {
      setSrc(undefined);
      return undefined;
    }
    const blob = new Blob([Uint8Array.from(renderer.bytes)], { type: renderer.mimeType });
    const nextSrc = URL.createObjectURL(blob);
    setSrc(nextSrc);
    return () => URL.revokeObjectURL(nextSrc);
  }, [renderer]);

  if (!src) {
    return (
      <p data-preview-image-unavailable role="status">
        Image preview is unavailable in this renderer.
      </p>
    );
  }

  return (
    <figure data-preview-image>
      <img alt="" src={src} />
      {renderer.truncated ? <figcaption>Preview content was truncated.</figcaption> : null}
    </figure>
  );
}
