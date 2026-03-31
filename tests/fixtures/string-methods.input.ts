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

// Slice
function getSubstring(s: string, start: number, end: number): string {
  return s.slice(start, end);
}

// IndexOf
function findPosition(s: string, target: string): number {
  return s.indexOf(target);
}

// Split
function splitWords(s: string): string[] {
  return s.split(" ");
}

// charAt
function firstChar(s: string): string {
  return s.charAt(0);
}

// Concat
function concat(a: string, b: string): string {
  return a + b;
}

// Repeat
function repeatStr(s: string, count: number): string {
  return s.repeat(count);
}

// Substring
function extractMiddle(s: string): string {
  return s.substring(1, 3);
}
