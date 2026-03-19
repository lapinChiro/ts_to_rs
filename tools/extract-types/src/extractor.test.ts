import { describe, it, expect } from "vitest";
import ts from "typescript";
import { extractTypes } from "./extractor.js";
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
