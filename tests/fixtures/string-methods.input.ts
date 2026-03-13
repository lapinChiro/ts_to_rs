function getLength(s: string): number {
  return s.length;
}

function checkPrefix(s: string): boolean {
  return s.startsWith("hello");
}

function normalize(s: string): string {
  return s.trim().toLowerCase();
}

function replaceChar(s: string): string {
  return s.replace("a", "b");
}

function hasContent(s: string): boolean {
  return s.includes("x") && !s.endsWith("z");
}

function shout(s: string): string {
  return s.toUpperCase();
}
