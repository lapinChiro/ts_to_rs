import { describe, it, expect } from "vitest";
import ts from "typescript";
import { extractTypes } from "./extractor.js";
import { filterTypes, ECMASCRIPT_TYPES, SERVER_WEB_API_TYPES } from "./filter.js";
import type { ExternalInterfaceDef, ExternalTypesJson } from "./types.js";

/** Helper: create a program from inline TypeScript source. */
function createProgramFromSource(
  source: string,
  libs: string[] = [],
): ts.Program {
  const fileName = "test.ts";
  const compilerOptions: ts.CompilerOptions = {
    target: ts.ScriptTarget.ES2024,
    lib: libs,
    noEmit: true,
    strict: true,
  };

  const host = ts.createCompilerHost(compilerOptions);
  const originalGetSourceFile = host.getSourceFile.bind(host);
  host.getSourceFile = (name, languageVersion, onError, shouldCreate) => {
    if (name === fileName) {
      return ts.createSourceFile(name, source, languageVersion);
    }
    return originalGetSourceFile(name, languageVersion, onError, shouldCreate);
  };
  host.fileExists = (name) => {
    if (name === fileName) return true;
    return ts.sys.fileExists(name);
  };
  host.readFile = (name) => {
    if (name === fileName) return source;
    return ts.sys.readFile(name);
  };

  return ts.createProgram([fileName], compilerOptions, host);
}

function extract(source: string, libs?: string[]): ExternalTypesJson {
  const program = createProgramFromSource(source, libs);
  return extractTypes(program);
}

function getInterface(
  result: ExternalTypesJson,
  name: string,
): ExternalInterfaceDef {
  const def = result.types[name];
  expect(def).toBeDefined();
  expect(def.kind).toBe("interface");
  return def as ExternalInterfaceDef;
}

describe("basic interface extraction", () => {
  it("extracts fields with correct types", () => {
    const result = extract("interface Foo { x: string; y: number; }");
    const foo = getInterface(result, "Foo");
    expect(foo.fields).toContainEqual(
      expect.objectContaining({ name: "x", type: { kind: "string" } }),
    );
    expect(foo.fields).toContainEqual(
      expect.objectContaining({ name: "y", type: { kind: "number" } }),
    );
  });

  it("marks optional fields", () => {
    const result = extract("interface Foo { x?: string; }");
    const foo = getInterface(result, "Foo");
    expect(foo.fields).toContainEqual(
      expect.objectContaining({
        name: "x",
        type: { kind: "string" },
        optional: true,
      }),
    );
  });

  it("marks readonly fields", () => {
    const result = extract("interface Foo { readonly x: string; }");
    const foo = getInterface(result, "Foo");
    expect(foo.fields).toContainEqual(
      expect.objectContaining({
        name: "x",
        type: { kind: "string" },
        readonly: true,
      }),
    );
  });

  it("extracts methods", () => {
    const result = extract(
      "interface Foo { bar(x: number): string; }",
    );
    const foo = getInterface(result, "Foo");
    expect(foo.methods["bar"]).toBeDefined();
    expect(foo.methods["bar"].signatures).toHaveLength(1);
    expect(foo.methods["bar"].signatures[0].params).toHaveLength(1);
    expect(foo.methods["bar"].signatures[0].params[0].name).toBe("x");
    expect(foo.methods["bar"].signatures[0].return_type).toEqual({
      kind: "string",
    });
  });
});

describe("inheritance flattening", () => {
  it("includes parent fields in child", () => {
    const result = extract(`
      interface Parent { x: string; }
      interface Child extends Parent { y: number; }
    `);
    const child = getInterface(result, "Child");
    const fieldNames = child.fields.map((f) => f.name);
    expect(fieldNames).toContain("x");
    expect(fieldNames).toContain("y");
  });

  it("includes parent methods in child", () => {
    const result = extract(`
      interface Parent { foo(): string; }
      interface Child extends Parent { bar(): number; }
    `);
    const child = getInterface(result, "Child");
    expect(child.methods["foo"]).toBeDefined();
    expect(child.methods["bar"]).toBeDefined();
  });

  it("handles multi-level inheritance", () => {
    const result = extract(`
      interface A { a: string; }
      interface B extends A { b: number; }
      interface C extends B { c: boolean; }
    `);
    const c = getInterface(result, "C");
    const fieldNames = c.fields.map((f) => f.name);
    expect(fieldNames).toContain("a");
    expect(fieldNames).toContain("b");
    expect(fieldNames).toContain("c");
  });
});

describe("generic defaults", () => {
  it("resolves generic default types", () => {
    const result = extract(`
      interface Container<T = string> { value: T; }
    `);
    // The type checker resolves T to string when no arg is provided
    // But the interface definition itself stores T as a type parameter
    const container = getInterface(result, "Container");
    expect(container.fields).toHaveLength(1);
    // T is unresolved at definition site; it becomes resolved at usage
  });
});

describe("conditional types", () => {
  it("resolves conditional type aliases", () => {
    const result = extract(`
      type IsString<T> = T extends string ? true : false;
      type Result = IsString<"hello">;
    `);
    // tsc resolves Result to `true` (boolean literal)
    expect(result.types["Result"]).toBeDefined();
  });
});

describe("utility types", () => {
  it("resolves Partial", () => {
    const result = extract(`
      interface Foo { x: string; y: number; }
      type PartialFoo = Partial<Foo>;
    `);
    expect(result.types["PartialFoo"]).toBeDefined();
    // Partial makes all fields optional; tsc resolves this
  });
});

describe("constructors", () => {
  it("extracts constructor from declare var pattern", () => {
    const result = extract(`
      interface Response { readonly status: number; }
      declare var Response: { new(body?: string): Response; };
    `);
    const resp = getInterface(result, "Response");
    expect(resp.constructors.length).toBeGreaterThan(0);
    expect(resp.constructors[0].params.length).toBeGreaterThan(0);
  });
});

describe("declaration merging", () => {
  it("merges multiple interface declarations", () => {
    const result = extract(`
      interface Foo { x: string; }
      interface Foo { y: number; }
    `);
    const foo = getInterface(result, "Foo");
    const fieldNames = foo.fields.map((f) => f.name);
    expect(fieldNames).toContain("x");
    expect(fieldNames).toContain("y");
  });
});

describe("overloads", () => {
  it("captures all overload signatures", () => {
    const result = extract(`
      interface Foo {
        bar(x: number): string;
        bar(x: string): number;
      }
    `);
    const foo = getInterface(result, "Foo");
    expect(foo.methods["bar"]).toBeDefined();
    expect(foo.methods["bar"].signatures.length).toBeGreaterThanOrEqual(2);
  });
});

describe("optional params and `T | undefined`", () => {
  // Step 2 (RC-2) guarantees: the Rust loader (external_types::convert_union_type)
  // converts `T | undefined` into `Option<T>`. If extraction either drops the
  // `| undefined` member or fails to set `optional: true`, the downstream conversion
  // either generates a less precise type (dropping optionality) or double-wraps
  // (if both the union AND the `optional` flag are present). The tests below
  // pin the contracts that keep the Rust loader's conversion lossless.

  it("detects optional params in interface method signatures via questionToken", () => {
    // `param.flags & ts.SymbolFlags.Optional` is NOT set for callable-signature
    // parameters declared with `?`. Step 2's extractor falls back to the AST
    // declaration's questionToken so these params are correctly flagged.
    const result = extract(`
      interface Foo {
        bar(required: string, position?: number): boolean;
      }
    `);
    const foo = getInterface(result, "Foo");
    const sig = foo.methods["bar"].signatures[0];
    expect(sig.params).toHaveLength(2);
    expect(sig.params[0]).toMatchObject({ name: "required", type: { kind: "string" } });
    expect(sig.params[0].optional).toBeUndefined();
    expect(sig.params[1]).toMatchObject({
      name: "position",
      type: { kind: "number" },
      optional: true,
    });
  });

  it("strips `| undefined` from optional param types so the Rust loader wraps only once", () => {
    // If extraction left `position` as `{ union [undefined, number] }` AND
    // emitted `optional: true`, the Rust side would apply Option twice, yielding
    // `Option<Option<f64>>`. Step 2's extractor calls stripUndefined for optional
    // params so the union collapses to its non-undefined member.
    const result = extract(`
      interface Foo {
        bar(position?: number): void;
      }
    `);
    const sig = getInterface(result, "Foo").methods["bar"].signatures[0];
    expect(sig.params[0].type).toEqual({ kind: "number" });
  });

  it("preserves `T | undefined` return types as explicit unions (find/pop pattern)", () => {
    // TypeScript's checker simplifies `S | undefined` back to `S` for generic
    // interface method return types when strictNullChecks is off. With
    // strictNullChecks on (enabled at the program level in production), the
    // union is preserved. extractSignature relies on `sig.getReturnType()` and
    // must therefore see the union — verify via a generic method.
    const result = extract(`
      interface MyArray<T> {
        find<S extends T>(predicate: (value: T) => value is S): S | undefined;
        pop(): T | undefined;
      }
    `);
    const arr = getInterface(result, "MyArray");
    const findRet = arr.methods["find"]!.signatures[0]!.return_type!;
    expect(findRet.kind).toBe("union");
    if (findRet.kind === "union") {
      const kinds = findRet.members.map((m) => m.kind).sort();
      expect(kinds).toEqual(["named", "undefined"]);
    }
    const popRet = arr.methods["pop"]!.signatures[0]!.return_type!;
    expect(popRet.kind).toBe("union");
    if (popRet.kind === "union") {
      const kinds = popRet.members.map((m) => m.kind).sort();
      expect(kinds).toEqual(["named", "undefined"]);
    }
  });

  it("keeps `T | undefined` from a non-optional param as a union (no strip)", () => {
    // stripUndefined must ONLY fire on optional params. A non-optional param
    // with an explicit `| undefined` type must retain the union so the Rust
    // loader produces `Option<T>` at the parameter site.
    const result = extract(`
      interface Foo {
        bar(maybe: string | undefined): void;
      }
    `);
    const sig = getInterface(result, "Foo").methods["bar"].signatures[0];
    const ty = sig.params[0].type;
    expect(sig.params[0].optional).toBeUndefined();
    expect(ty.kind).toBe("union");
    if (ty.kind === "union") {
      const kinds = ty.members.map((m) => m.kind).sort();
      expect(kinds).toEqual(["string", "undefined"]);
    }
  });
});

describe("lib.dom.d.ts integration", { timeout: 30_000 }, () => {
  it("extracts Response with inherited Body methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.dom.d.ts",
      "lib.es2024.d.ts",
    ]);
    const result = extractTypes(program);

    // Response should exist
    expect(result.types["Response"]).toBeDefined();
    const response = result.types["Response"] as ExternalInterfaceDef;
    expect(response.kind).toBe("interface");

    // Should have own fields
    const fieldNames = response.fields.map((f) => f.name);
    expect(fieldNames).toContain("status");
    expect(fieldNames).toContain("ok");
    expect(fieldNames).toContain("headers");

    // Should have inherited methods from Body
    expect(response.methods["json"]).toBeDefined();
    expect(response.methods["text"]).toBeDefined();
    expect(response.methods["arrayBuffer"]).toBeDefined();

    // Should have constructors
    expect(response.constructors.length).toBeGreaterThan(0);
  });

  it("extracts Request", () => {
    const program = createProgramFromSource("export {};", [
      "lib.dom.d.ts",
      "lib.es2024.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Request"]).toBeDefined();
    const request = result.types["Request"] as ExternalInterfaceDef;
    const fieldNames = request.fields.map((f) => f.name);
    expect(fieldNames).toContain("url");
    expect(fieldNames).toContain("method");
    expect(fieldNames).toContain("headers");
  });

  it("extracts Headers", () => {
    const program = createProgramFromSource("export {};", [
      "lib.dom.d.ts",
      "lib.es2024.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Headers"]).toBeDefined();
    const headers = result.types["Headers"] as ExternalInterfaceDef;
    expect(headers.methods["get"]).toBeDefined();
    expect(headers.methods["set"]).toBeDefined();
    expect(headers.methods["append"]).toBeDefined();
  });
});

describe("lib.es5.d.ts integration", { timeout: 30_000 }, () => {
  it("extracts String with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["String"]).toBeDefined();
    const str = result.types["String"] as ExternalInterfaceDef;
    expect(str.kind).toBe("interface");
    expect(str.methods["trim"]).toBeDefined();
    expect(str.methods["split"]).toBeDefined();
    expect(str.methods["toLowerCase"]).toBeDefined();
    expect(str.methods["toUpperCase"]).toBeDefined();
    expect(str.methods["indexOf"]).toBeDefined();
    expect(str.methods["slice"]).toBeDefined();
    expect(str.methods["replace"]).toBeDefined();
  });

  it("extracts String.split with array return type", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    const str = result.types["String"] as ExternalInterfaceDef;
    const splitSigs = str.methods["split"].signatures;
    expect(splitSigs.length).toBeGreaterThan(0);
    // At least one signature should return string[]
    const hasArrayReturn = splitSigs.some(
      (sig) =>
        sig.return_type?.kind === "array" &&
        sig.return_type.element.kind === "string",
    );
    expect(hasArrayReturn).toBe(true);
  });

  it("extracts Array with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Array"]).toBeDefined();
    const arr = result.types["Array"] as ExternalInterfaceDef;
    expect(arr.kind).toBe("interface");
    expect(arr.methods["map"]).toBeDefined();
    expect(arr.methods["filter"]).toBeDefined();
    expect(arr.methods["indexOf"]).toBeDefined();
    expect(arr.methods["push"]).toBeDefined();
    expect(arr.methods["pop"]).toBeDefined();
    expect(arr.methods["join"]).toBeDefined();
    expect(arr.methods["slice"]).toBeDefined();
  });

  it("extracts Date with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Date"]).toBeDefined();
    const date = result.types["Date"] as ExternalInterfaceDef;
    expect(date.kind).toBe("interface");
    expect(date.methods["getTime"]).toBeDefined();
    expect(date.methods["toISOString"]).toBeDefined();
  });

  it("extracts Error with message field", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Error"]).toBeDefined();
    const error = result.types["Error"] as ExternalInterfaceDef;
    expect(error.kind).toBe("interface");
    const fieldNames = error.fields.map((f) => f.name);
    expect(fieldNames).toContain("message");
  });

  it("extracts RegExp with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["RegExp"]).toBeDefined();
    const regexp = result.types["RegExp"] as ExternalInterfaceDef;
    expect(regexp.kind).toBe("interface");
    expect(regexp.methods["test"]).toBeDefined();
    expect(regexp.methods["exec"]).toBeDefined();
  });

  it("extracts JSON with parse and stringify", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["JSON"]).toBeDefined();
    const json = result.types["JSON"] as ExternalInterfaceDef;
    expect(json.kind).toBe("interface");
    expect(json.methods["parse"]).toBeDefined();
    expect(json.methods["stringify"]).toBeDefined();
  });

  it("extracts Math with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Math"]).toBeDefined();
    const math = result.types["Math"] as ExternalInterfaceDef;
    expect(math.kind).toBe("interface");
    expect(math.methods["floor"]).toBeDefined();
    expect(math.methods["ceil"]).toBeDefined();
    expect(math.methods["round"]).toBeDefined();
    expect(math.methods["max"]).toBeDefined();
    expect(math.methods["min"]).toBeDefined();
    expect(math.methods["random"]).toBeDefined();
  });
});

describe("lib.es2015 integration", { timeout: 30_000 }, () => {
  it("extracts Map with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es2015.collection.d.ts",
      "lib.es2015.iterable.d.ts",
      "lib.es2015.symbol.d.ts",
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Map"]).toBeDefined();
    const map = result.types["Map"] as ExternalInterfaceDef;
    expect(map.kind).toBe("interface");
    expect(map.methods["get"]).toBeDefined();
    expect(map.methods["set"]).toBeDefined();
    expect(map.methods["has"]).toBeDefined();
    expect(map.methods["delete"]).toBeDefined();
  });

  it("extracts Set with core methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es2015.collection.d.ts",
      "lib.es2015.iterable.d.ts",
      "lib.es2015.symbol.d.ts",
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Set"]).toBeDefined();
    const set = result.types["Set"] as ExternalInterfaceDef;
    expect(set.kind).toBe("interface");
    expect(set.methods["add"]).toBeDefined();
    expect(set.methods["has"]).toBeDefined();
    expect(set.methods["delete"]).toBeDefined();
  });

  it("extracts Promise with then/catch methods", () => {
    const program = createProgramFromSource("export {};", [
      "lib.es2015.promise.d.ts",
      "lib.es2015.iterable.d.ts",
      "lib.es2015.symbol.d.ts",
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);

    expect(result.types["Promise"]).toBeDefined();
    const promise = result.types["Promise"] as ExternalInterfaceDef;
    expect(promise.kind).toBe("interface");
    expect(promise.methods["then"]).toBeDefined();
    expect(promise.methods["catch"]).toBeDefined();
  });
});

describe("ECMAScript filter", { timeout: 30_000 }, () => {
  it("includes only ECMAScript types and excludes DOM types", () => {
    // Extract from both ES5 and DOM
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
      "lib.dom.d.ts",
    ]);
    const raw = extractTypes(program);

    // DOM types should be present in raw
    expect(raw.types["HTMLElement"]).toBeDefined();

    // Apply ECMAScript filter
    const filtered = filterTypes(raw, ECMASCRIPT_TYPES);

    // ECMAScript types should be present
    expect(filtered.types["String"]).toBeDefined();
    expect(filtered.types["Array"]).toBeDefined();
    expect(filtered.types["Date"]).toBeDefined();
    expect(filtered.types["Error"]).toBeDefined();
    expect(filtered.types["Math"]).toBeDefined();
    expect(filtered.types["JSON"]).toBeDefined();

    // DOM types should be excluded
    expect(filtered.types["HTMLElement"]).toBeUndefined();
    expect(filtered.types["Document"]).toBeUndefined();
    expect(filtered.types["Window"]).toBeUndefined();
    expect(filtered.types["Response"]).toBeUndefined();
  });

  it("ECMASCRIPT_TYPES includes expected root types", () => {
    expect(ECMASCRIPT_TYPES).toContain("String");
    expect(ECMASCRIPT_TYPES).toContain("Number");
    expect(ECMASCRIPT_TYPES).toContain("Array");
    expect(ECMASCRIPT_TYPES).toContain("Date");
    expect(ECMASCRIPT_TYPES).toContain("Error");
    expect(ECMASCRIPT_TYPES).toContain("RegExp");
    expect(ECMASCRIPT_TYPES).toContain("Map");
    expect(ECMASCRIPT_TYPES).toContain("Set");
    expect(ECMASCRIPT_TYPES).toContain("WeakMap");
    expect(ECMASCRIPT_TYPES).toContain("WeakSet");
    expect(ECMASCRIPT_TYPES).toContain("Symbol");
    expect(ECMASCRIPT_TYPES).toContain("Promise");
    expect(ECMASCRIPT_TYPES).toContain("JSON");
    expect(ECMASCRIPT_TYPES).toContain("Math");
  });
});

describe("anonymous type literal (__type handling)", () => {
  it("converts anonymous call signature type to function type instead of __type", () => {
    // TypeScript creates an internal __type symbol for anonymous type literals
    // used as method parameter types. The extractor should expand these to
    // their actual structure (function type) instead of emitting { kind: "named", name: "__type" }.
    //
    // This mimics the real pattern: String.match(matcher: { [Symbol.match](s: string): T })
    // where the parameter type is an anonymous type literal with a call signature.
    const result = extract(`
      interface Foo {
        bar(matcher: { (s: string): boolean }): boolean;
      }
    `);
    const foo = getInterface(result, "Foo");
    const barSig = foo.methods["bar"].signatures[0];
    const matcherParam = barSig.params[0];
    // Should be expanded to function type, NOT { kind: "named", name: "__type" }
    expect(matcherParam.type).not.toEqual(
      expect.objectContaining({ kind: "named", name: "__type" }),
    );
    expect(matcherParam.type.kind).toBe("function");
  });

  it("converts anonymous index signature type to Record instead of __type", () => {
    // { [key: string]: string } should become Record<string, string>,
    // not { kind: "named", name: "__type" }.
    const result = extract(`
      interface RegExpExecArray {
        groups?: { [key: string]: string };
      }
    `);
    const arr = getInterface(result, "RegExpExecArray");
    const groupsField = arr.fields.find((f) => f.name === "groups");
    expect(groupsField).toBeDefined();
    expect(groupsField!.type).toEqual({
      kind: "named",
      name: "Record",
      type_args: [{ kind: "string" }, { kind: "string" }],
    });
  });

  it("converts anonymous Symbol-keyed property type to function instead of __type", () => {
    // { [Symbol.match](s: string): boolean } has a Symbol-keyed property
    // with a call signature. Should be extracted as function type.
    const result = extract(`
      interface Foo {
        match(matcher: { [Symbol.match](s: string): boolean }): boolean;
      }
    `);
    const foo = getInterface(result, "Foo");
    const matchSig = foo.methods["match"].signatures[0];
    const matcherParam = matchSig.params[0];
    expect(matcherParam.type).not.toEqual(
      expect.objectContaining({ kind: "named", name: "__type" }),
    );
    expect(matcherParam.type.kind).toBe("function");
  });

  it("never produces __type in any output for lib.es5 types", { timeout: 30_000 }, () => {
    // Regression guard: ensure no __type leaks in real lib.es5 extraction
    const program = createProgramFromSource("export {};", [
      "lib.es5.d.ts",
    ]);
    const result = extractTypes(program);
    const jsonStr = JSON.stringify(result);
    expect(jsonStr).not.toContain('"__type"');
  });
});

describe("symbol primitive handling", () => {
  it("converts ESSymbol type to any instead of named symbol", () => {
    const result = extract(`
      interface Foo {
        bar(x: symbol): void;
      }
    `);
    const foo = getInterface(result, "Foo");
    const barSig = foo.methods["bar"].signatures[0];
    // symbol should be { kind: "any" }, NOT { kind: "named", name: "symbol" }
    expect(barSig.params[0].type).toEqual({ kind: "any" });
  });
});

describe("filter referential integrity", () => {
  it("adds empty definitions for excluded-but-referenced types", { timeout: 30_000 }, () => {
    const program = createProgramFromSource("export {};", [
      "lib.dom.d.ts",
      "lib.es2024.d.ts",
    ]);
    const raw = extractTypes(program);
    const filtered = filterTypes(raw, SERVER_WEB_API_TYPES);

    // WebSocket is a root type → should be included
    expect(filtered.types["WebSocket"]).toBeDefined();

    // Specific excluded types that are referenced by included types
    // should have empty stub definitions
    const excludedButReferenced = [
      "Window",           // Referenced by MessageEvent.source union
      "ServiceWorker",    // Referenced by MessageEvent.source union
    ];

    for (const name of excludedButReferenced) {
      expect(filtered.types[name]).toBeDefined();
      // Should be an empty interface stub
      const def = filtered.types[name];
      expect(def.kind).toBe("interface");
      if (def.kind === "interface") {
        expect(def.fields).toHaveLength(0);
      }
    }
  });
});
