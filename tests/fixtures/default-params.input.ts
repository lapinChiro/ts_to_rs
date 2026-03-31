// Default parameters

// String default
function greet(name: string = "world"): string {
  return name;
}

// Number default
function add(a: number, b: number = 0): number {
  return a + b;
}

// Boolean default
function toggle(flag: boolean = false): boolean {
  return !flag;
}

// Multiple defaults
function createLabel(text: string = "label", size: number = 12, bold: boolean = false): string {
  return `${text} (${size}${bold ? " bold" : ""})`;
}

// Default with expression
function offset(x: number, delta: number = 1): number {
  return x + delta;
}

// Mix of required and default
function formatName(first: string, last: string, prefix: string = "Mr."): string {
  return `${prefix} ${first} ${last}`;
}

// Array default
function process(items: string[], separator: string = ","): string {
  return items.join(separator);
}

// Object type default value
interface ServerConfig {
  host: string;
  port: number;
}

function startServer(config: ServerConfig = { host: "localhost", port: 8080 }): string {
  return `${config.host}:${config.port}`;
}
