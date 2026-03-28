/**
 * Filters extracted types to only include specified root types
 * and their transitive dependencies.
 */

import type { ExternalTypesJson, ExternalTypeDef, ExternalType } from "./types.js";

/** ECMAScript standard built-in types (ES5 + ES2015). */
export const ECMASCRIPT_TYPES: string[] = [
  // ES5 core
  "String", "Number", "Boolean", "Object", "Function",
  "Array", "Date", "Error", "RegExp", "JSON", "Math",
  // ES5 error types
  "TypeError", "RangeError", "SyntaxError", "ReferenceError", "EvalError", "URIError",
  // ES2015 collections
  "Map", "Set", "WeakMap", "WeakSet",
  // ES2015 other
  "Symbol", "Promise",
  // TypedArray
  "ArrayBuffer", "DataView",
  "Int8Array", "Uint8Array", "Uint8ClampedArray",
  "Int16Array", "Uint16Array", "Int32Array", "Uint32Array",
  "Float32Array", "Float64Array",
  // Iterator/Generator
  "Iterator", "Generator", "GeneratorFunction",
  "IterableIterator", "IteratorResult",
];

/** Server-side Web API types needed for Hono and similar frameworks. */
export const SERVER_WEB_API_TYPES = [
  // Fetch API
  "Request",
  "Response",
  "Headers",
  "RequestInit",
  "ResponseInit",
  "HeadersInit",
  "RequestInfo",
  "BodyInit",
  "Body",
  "ReferrerPolicy",
  "RequestCache",
  "RequestCredentials",
  "RequestDestination",
  "RequestMode",
  "RequestRedirect",
  "ResponseType",

  // URL
  "URL",
  "URLSearchParams",

  // Streams
  "ReadableStream",
  "WritableStream",
  "TransformStream",
  "ReadableStreamDefaultReader",
  "ReadableStreamDefaultController",
  "WritableStreamDefaultWriter",

  // Crypto
  "Crypto",
  "SubtleCrypto",
  "CryptoKey",
  "CryptoKeyPair",
  "Algorithm",
  "AlgorithmIdentifier",

  // Form data / Blob
  "FormData",
  "Blob",
  "File",
  "BlobPropertyBag",

  // Abort
  "AbortController",
  "AbortSignal",

  // Events
  "Event",
  "EventTarget",
  "EventInit",
  "EventListener",
  "EventListenerOrEventListenerObject",

  // Text encoding
  "TextEncoder",
  "TextDecoder",
  "TextDecoderOptions",
  "TextDecodeOptions",

  // Timers (global functions used in server code)
  "setTimeout",
  "clearTimeout",
  "setInterval",
  "clearInterval",

  // Global functions
  "fetch",

  // WebSocket
  "WebSocket",
  "CloseEvent",

  // Console
  "Console",
];

/** Type names to exclude from transitive dependency tracking. */
const EXCLUDE_PATTERNS = [
  /^HTML/,
  /^SVG/,
  /^CSS/,
  /^WebGL/,
  /^WEBGL/,
  /^IDB/,
  /^RTC/,
  /^MIDI/,
  /^OES_/,
  /^EXT_/,
  /^KHR_/,
  /^OVR_/,
  /^ANGLE_/,
  /^Canvas/,
  /^Gamepad/,
  /^Media(?!Encrypt)/,    // Keep MediaEncrypted but exclude MediaStream etc.
  /^Audio/,
  /^Video(?!Color)/,
  /^Speech/,
  /^Animation(?!Event)/,
  /^Notification/,
  /^Navigator$/,
  /^Window$/,
  /^Document$/,
  /^Element$/,
  /^Node$/,
  /^Range$/,
  /^Selection$/,
  /^Plugin/,
  /^MimeType/,
  /^Screen/,
  /^History$/,
  /^Location$/,
  /^Storage(?!Manager|Estimate)/,
  /^Performance/,
  /^Worker$/,
  /^ServiceWorker/,
  /^Push/,
  /^Cache$/,
  /^CacheStorage$/,
  /^Cookie/,
  /^Lock(?!Info)/,
  /^Remote/,
  /^Shadow/,
  /^Source/,
  /^Font/,
  /^Intersection/,
  /^Mutation/,
  /^Resize/,
  /^Drag/,
  /^Clipboard/,
  /^Pointer/,
  /^Touch/,
  /^Wheel/,
  /^Keyboard/,
  /^Mouse/,
  /^Focus/,
  /^Input/,
  /^Composition/,
  /^Transition/,
  /^Idle/,
  /^Image(?!Data)/,
];

function isExcluded(name: string): boolean {
  return EXCLUDE_PATTERNS.some((pattern) => pattern.test(name));
}

/**
 * Filters the extracted types to only include the specified roots
 * and all types transitively referenced by them (excluding DOM/browser types).
 */
export function filterTypes(
  json: ExternalTypesJson,
  roots: string[],
): ExternalTypesJson {
  const needed = new Set<string>();
  const queue = [...roots];

  while (queue.length > 0) {
    const name = queue.pop()!;
    if (needed.has(name)) continue;
    // Allow root types even if they match exclude patterns
    if (!roots.includes(name) && isExcluded(name)) continue;
    const def = json.types[name];
    if (!def) continue;
    needed.add(name);

    // Collect referenced type names
    for (const ref of collectReferences(def)) {
      if (!needed.has(ref)) {
        queue.push(ref);
      }
    }
  }

  const filtered: Record<string, ExternalTypeDef> = {};
  for (const name of needed) {
    if (json.types[name]) {
      filtered[name] = json.types[name];
    }
  }

  return { version: json.version, types: filtered };
}

/** Collects all type names referenced by a type definition. */
function collectReferences(def: ExternalTypeDef): Set<string> {
  const refs = new Set<string>();

  if (def.kind === "interface") {
    for (const field of def.fields) {
      collectTypeRefs(field.type, refs);
    }
    for (const method of Object.values(def.methods)) {
      for (const sig of method.signatures) {
        for (const param of sig.params) {
          collectTypeRefs(param.type, refs);
        }
        if (sig.return_type) {
          collectTypeRefs(sig.return_type, refs);
        }
      }
    }
    for (const ctor of def.constructors) {
      for (const param of ctor.params) {
        collectTypeRefs(param.type, refs);
      }
      if (ctor.return_type) {
        collectTypeRefs(ctor.return_type, refs);
      }
    }
  } else if (def.kind === "function") {
    for (const sig of def.signatures) {
      for (const param of sig.params) {
        collectTypeRefs(param.type, refs);
      }
      if (sig.return_type) {
        collectTypeRefs(sig.return_type, refs);
      }
    }
  } else if (def.kind === "alias") {
    collectTypeRefs(def.type, refs);
  }

  return refs;
}

function collectTypeRefs(type: ExternalType, refs: Set<string>): void {
  switch (type.kind) {
    case "named":
      refs.add(type.name);
      if (type.type_args) {
        for (const arg of type.type_args) {
          collectTypeRefs(arg, refs);
        }
      }
      break;
    case "array":
      collectTypeRefs(type.element, refs);
      break;
    case "tuple":
      for (const el of type.elements) {
        collectTypeRefs(el, refs);
      }
      break;
    case "union":
      for (const m of type.members) {
        collectTypeRefs(m, refs);
      }
      break;
    case "function":
      for (const p of type.params) {
        collectTypeRefs(p, refs);
      }
      collectTypeRefs(type.return_type, refs);
      break;
  }
}
