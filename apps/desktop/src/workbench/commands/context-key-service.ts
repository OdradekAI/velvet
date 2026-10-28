export type ContextKeyValue = string | number | boolean | null;
export type ContextKeySnapshot = Readonly<Record<string, ContextKeyValue>>;

export class ContextKeyService {
  private readonly values = new Map<string, ContextKeyValue>();
  private readonly listeners = new Set<(snapshot: ContextKeySnapshot) => void>();

  public snapshot(): ContextKeySnapshot {
    return Object.fromEntries(this.values.entries());
  }

  public set(key: string, value: ContextKeyValue | undefined): void {
    if (value === undefined) {
      this.values.delete(key);
    } else {
      this.values.set(key, value);
    }

    const snapshot = this.snapshot();
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }

  public subscribe(listener: (snapshot: ContextKeySnapshot) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public evaluate(
    expression: string | undefined,
    snapshot: ContextKeySnapshot = this.snapshot(),
  ): boolean {
    if (!expression?.trim()) {
      return true;
    }

    return expression
      .split('&&')
      .map((term) => term.trim())
      .every((term) => evaluateTerm(term, snapshot));
  }
}

function evaluateTerm(term: string, snapshot: ContextKeySnapshot): boolean {
  if (term.startsWith('!') && !term.includes('!=')) {
    return !Boolean(snapshot[term.slice(1).trim()]);
  }

  const comparison = /^(?<key>[A-Za-z0-9._-]+)\s*(?<operator>==|!=)\s*(?<value>.+)$/.exec(
    term,
  );
  if (comparison?.groups) {
    const left = snapshot[comparison.groups.key];
    const right = parseValue(comparison.groups.value.trim());
    return comparison.groups.operator === '==' ? left === right : left !== right;
  }

  return Boolean(snapshot[term]);
}

function parseValue(value: string): ContextKeyValue {
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1);
  }
  if (value === 'true') {
    return true;
  }
  if (value === 'false') {
    return false;
  }
  if (value === 'null') {
    return null;
  }

  const number = Number(value);
  return Number.isNaN(number) ? value : number;
}
