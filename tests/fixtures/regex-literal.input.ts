// Regex literal patterns

function makePattern(): number {
  const pattern = /hello/;
  const count: number = 1;
  return count;
}

function makeInsensitivePattern(): number {
  const pattern = /hello/i;
  const count: number = 2;
  return count;
}

// Global flag
function makeGlobalPattern(): number {
  const pattern = /hello/g;
  const count: number = 3;
  return count;
}

// Multiple flags
function makeMultiFlagPattern(): number {
  const pattern = /hello/gi;
  const count: number = 4;
  return count;
}

// Special characters
function makeSpecialPattern(): number {
  const pattern = /\d+\.\d+/;
  const count: number = 5;
  return count;
}

// Regex with character classes
function makeDigitPattern(): number {
  const pattern = /[0-9]+/;
  const count: number = 6;
  return count;
}
