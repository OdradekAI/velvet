
export function VelvetSeparator({ orientation = 'horizontal' }: { readonly orientation?: 'horizontal' | 'vertical' }) {
  return <div aria-orientation={orientation} className={`velvet-separator velvet-separator--${orientation}`} role="separator" />;
}
