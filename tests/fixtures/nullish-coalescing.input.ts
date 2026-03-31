// Nullish coalescing: ?? operator

// Basic null fallback
function withDefault(x: number | null): number {
  return x ?? 0;
}

// Undefined fallback
function withFallback(name: string | undefined): string {
  return name ?? "anonymous";
}

// Chained nullish coalescing
function firstAvailable(a: string | null, b: string | null, c: string): string {
  return a ?? b ?? c;
}

// In variable assignment
function processConfig(timeout: number | null): number {
  const t = timeout ?? 3000;
  return t;
}

// With array index result
function getOrDefault(items: string[], index: number): string {
  return items[index] ?? "missing";
}

// Nullish assignment operator ??=
function ensureDefault(x: number | null): number {
  let val = x;
  val ??= 0;
  return val;
}
