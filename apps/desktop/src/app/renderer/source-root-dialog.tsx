import { useEffect, useState, type FormEvent } from 'react';
import type { SourceRootService } from '../../workbench/services/source-root-service.js';
import { VelvetDialog } from '../ui/velvet-ui.js';

export interface SourceRootDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly sourceRoots: SourceRootService;
  readonly pickDirectory?: () => Promise<string | undefined>;
  readonly confirmRevoke?: (root: string) => Promise<boolean>;
  readonly readOnly?: boolean;
}

/** Host-owned settings for the Rust source-location allow-list. */
export function SourceRootDialog({
  open,
  onOpenChange,
  sourceRoots,
  pickDirectory,
  confirmRevoke,
  readOnly = false,
}: SourceRootDialogProps) {
  const [roots, setRoots] = useState<readonly string[]>(sourceRoots.roots());
  const [rootInput, setRootInput] = useState('');
  const [busyRoot, setBusyRoot] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    if (!open) {
      return undefined;
    }

    setRoots(sourceRoots.roots());
    const unsubscribe = sourceRoots.subscribe(setRoots);
    void refresh();
    return unsubscribe;

    async function refresh(): Promise<void> {
      setLoading(true);
      setError(undefined);
      try {
        await sourceRoots.load();
      } catch (reason) {
        setError(errorMessage(reason));
      } finally {
        setLoading(false);
      }
    }
  }, [open, sourceRoots]);

  async function chooseDirectory(): Promise<void> {
    if (!pickDirectory || readOnly) {
      return;
    }
    setError(undefined);
    try {
      const selected = await pickDirectory();
      if (selected) {
        setRootInput(selected);
      }
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }

  async function addRoot(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (readOnly) {
      return;
    }
    const root = rootInput.trim();
    if (!root) {
      setError('Choose a folder or enter an absolute path.');
      return;
    }

    setBusyRoot(root);
    setError(undefined);
    setMessage(undefined);
    try {
      await sourceRoots.add(root);
      setRootInput('');
      setMessage(`Approved source folder: ${root}`);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusyRoot(undefined);
    }
  }

  async function revokeRoot(root: string): Promise<void> {
    if (readOnly) {
      return;
    }
    if (confirmRevoke && !(await confirmRevoke(root))) {
      return;
    }

    setBusyRoot(root);
    setError(undefined);
    setMessage(undefined);
    try {
      await sourceRoots.remove(root);
      setMessage(`Revoked source folder: ${root}`);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusyRoot(undefined);
    }
  }

  return (
    <VelvetDialog
      description="Approved folders let Velvet keep source locations across restart. Revoking a folder preserves Asset identity and blocks reads until it is approved again."
      onOpenChange={onOpenChange}
      open={open}
      title="Source folders"
    >
      <section aria-label="Approved source folders" className="velvet-source-roots" data-source-root-dialog>
        <div className="velvet-source-roots__toolbar">
          <span className="velvet-plugin-manager__eyebrow">Approved folders</span>
          <button
            className="velvet-button velvet-button--quiet"
            disabled={readOnly || loading}
            onClick={() => void sourceRoots.load().catch((reason) => setError(errorMessage(reason)))}
            type="button"
          >
            {loading ? 'Refreshing…' : 'Refresh'}
          </button>
        </div>
        <form className="velvet-source-roots__form" onSubmit={(event) => void addRoot(event)}>
          <label htmlFor="velvet-source-root-input">Folder path</label>
          <div className="velvet-source-roots__input-row">
            <input
              aria-describedby="velvet-source-root-help"
              className="velvet-input"
              disabled={readOnly || busyRoot !== undefined}
              id="velvet-source-root-input"
              onChange={(event) => setRootInput(event.currentTarget.value)}
              placeholder="/Users/name/Library/Assets"
              value={rootInput}
            />
            <button
              className="velvet-button velvet-button--quiet"
              disabled={readOnly || busyRoot !== undefined || pickDirectory === undefined}
              onClick={() => void chooseDirectory()}
              type="button"
            >
              Browse
            </button>
          </div>
          <p className="velvet-source-roots__help" id="velvet-source-root-help">
            Velvet checks the canonical directory and keeps this policy in the Rust source owner.
          </p>
          <button
            className="velvet-button"
            disabled={readOnly || busyRoot !== undefined || rootInput.trim().length === 0}
            type="submit"
          >
            {busyRoot && busyRoot === rootInput.trim() ? 'Approving…' : 'Approve folder'}
          </button>
        </form>
        <div aria-live="polite" className="velvet-source-roots__list" data-source-root-list>
          {roots.length === 0 ? <p className="velvet-view-placeholder">No approved folders.</p> : null}
          {roots.map((root) => (
            <div className="velvet-source-roots__item" data-source-root={root} key={root}>
              <code title={root}>{root}</code>
              <button
                aria-label={`Revoke ${root}`}
                className="velvet-button velvet-button--quiet velvet-button--danger"
                disabled={readOnly || busyRoot !== undefined}
                onClick={() => void revokeRoot(root)}
                type="button"
              >
                {busyRoot === root ? 'Revoking…' : 'Revoke'}
              </button>
            </div>
          ))}
        </div>
        {error ? <p aria-live="assertive" className="velvet-command-error" role="alert">{error}</p> : null}
        {message ? <p aria-live="polite" className="velvet-plugin-manager__success" role="status">{message}</p> : null}
      </section>
    </VelvetDialog>
  );
}

function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}
