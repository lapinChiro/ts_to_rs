/**
 * Core type extraction logic using TypeScript Compiler API.
 *
 * Walks all declarations in the program's source/lib files and
 * extracts fully-resolved type information into the JSON interchange format.
 */

import ts from "typescript";
import type {
  ExternalType,
  ExternalTypeDef,
  ExternalTypesJson,
  ExternalField,
  ExternalMethod,
  ExternalSignature,
  ExternalParam,
  ExternalInterfaceDef,
  ExternalTypeParam,
} from "./types.js";

/**
 * Extracts type information from all non-external source files in the program.
 *
 * For `--lib` mode, this processes the lib .d.ts files.
 * For `--tsconfig` mode, this processes the project's source files.
 */
export function extractTypes(program: ts.Program): ExternalTypesJson {
  const checker = program.getTypeChecker();
  const types: Record<string, ExternalTypeDef> = {};

  for (const sourceFile of program.getSourceFiles()) {
    // Skip the dummy file used in --lib mode
    if (sourceFile.fileName.includes("__extract_types_dummy__")) continue;

    // Process lib files (lib.dom.d.ts etc.) and project files
    // Skip node_modules but include lib files from TypeScript itself
    if (
      sourceFile.fileName.includes("node_modules") &&
      !sourceFile.fileName.includes("typescript/lib/")
    ) {
      continue;
    }

    ts.forEachChild(sourceFile, (node) => {
      processNode(node, checker, types);
    });
  }

  return { version: 2, types };
}

function processNode(
  node: ts.Node,
  checker: ts.TypeChecker,
  types: Record<string, ExternalTypeDef>,
): void {
  if (ts.isInterfaceDeclaration(node)) {
    const name = node.name.text;
    const def = extractInterface(node, checker, types[name]);
    types[name] = def;
  } else if (ts.isClassDeclaration(node) && node.name) {
    const name = node.name.text;
    types[name] = extractClass(node, checker);
  } else if (ts.isFunctionDeclaration(node) && node.name) {
    const name = node.name.text;
    types[name] = extractFunction(node, checker);
  } else if (ts.isTypeAliasDeclaration(node)) {
    const name = node.name.text;
    const resolvedType = checker.getTypeAtLocation(node);
    types[name] = extractTypeAlias(resolvedType, checker);
  } else if (ts.isVariableStatement(node)) {
    // Handle `declare var Response: { new(...): Response; ... }`
    for (const decl of node.declarationList.declarations) {
      if (ts.isIdentifier(decl.name)) {
        const name = decl.name.text;
        const existing = types[name];
        if (existing && existing.kind === "interface" && decl.type) {
          // Merge constructor signatures from the declare var into the existing interface
          mergeConstructors(existing, decl.type, checker);
        }
      }
    }
  } else if (ts.isModuleDeclaration(node) && node.body) {
    // Handle `declare global { ... }` and `declare namespace X { ... }`
    processModuleBody(node.body, checker, types);
  }
}

function processModuleBody(
  body: ts.ModuleBody,
  checker: ts.TypeChecker,
  types: Record<string, ExternalTypeDef>,
): void {
  if (ts.isModuleBlock(body)) {
    for (const statement of body.statements) {
      processNode(statement, checker, types);
    }
  } else if (ts.isModuleDeclaration(body) && body.body) {
    processModuleBody(body.body, checker, types);
  }
}

function extractInterface(
  node: ts.InterfaceDeclaration,
  checker: ts.TypeChecker,
  existing?: ExternalTypeDef,
): ExternalInterfaceDef {
  const type = checker.getTypeAtLocation(node);

  // Start from existing definition (for declaration merging)
  const fields: ExternalField[] =
    existing?.kind === "interface" ? [...existing.fields] : [];
  const methods: Record<string, ExternalMethod> =
    existing?.kind === "interface" ? { ...existing.methods } : {};
  const constructors: ExternalSignature[] =
    existing?.kind === "interface" ? [...existing.constructors] : [];

  const existingFieldNames = new Set(fields.map((f) => f.name));
  const existingMethodNames = new Set(Object.keys(methods));

  // Get all properties including inherited ones (checker resolves extends)
  for (const prop of type.getProperties()) {
    const propName = prop.getName();
    // Skip Symbol-keyed properties (tsc internal names like __@iterator@35)
    if (isSymbolProperty(propName)) continue;

    const propType = checker.getTypeOfSymbol(prop);
    const propDecl = prop.getDeclarations()?.[0];

    // Check if this is a method
    const callSignatures = propType.getCallSignatures();
    if (callSignatures.length > 0 && !existingMethodNames.has(propName)) {
      methods[propName] = {
        signatures: callSignatures.map((sig) =>
          extractSignature(sig, checker),
        ),
      };
    } else if (callSignatures.length === 0 && !existingFieldNames.has(propName)) {
      const isOptional = !!(prop.flags & ts.SymbolFlags.Optional);
      const isReadonly = propDecl
        ? hasReadonlyModifier(propDecl)
        : false;
      // For optional fields, tsc reports `T | undefined` — strip the `undefined`
      // to get the clean base type, since we represent optionality via the `optional` flag.
      const fieldType = isOptional
        ? convertType(stripUndefined(propType, checker), checker)
        : convertType(propType, checker);
      fields.push({
        name: propName,
        type: fieldType,
        ...(isOptional ? { optional: true } : {}),
        ...(isReadonly ? { readonly: true } : {}),
      });
    }
  }

  // Extract construct signatures
  for (const sig of type.getConstructSignatures()) {
    constructors.push(extractSignature(sig, checker));
  }

  // Extract type parameters from AST node (not resolved type)
  const typeParams = extractTypeParams(node.typeParameters, checker);

  // For declaration merging, keep type_params from first declaration only
  const existingTypeParams = existing?.kind === "interface" ? existing.type_params : undefined;
  const finalTypeParams = existingTypeParams ?? (typeParams.length > 0 ? typeParams : undefined);

  return {
    kind: "interface",
    ...(finalTypeParams ? { type_params: finalTypeParams } : {}),
    fields,
    methods,
    constructors,
  };
}

function extractClass(
  node: ts.ClassDeclaration,
  checker: ts.TypeChecker,
): ExternalInterfaceDef {
  const type = checker.getTypeAtLocation(node);

  const fields: ExternalField[] = [];
  const methods: Record<string, ExternalMethod> = {};
  const constructors: ExternalSignature[] = [];

  for (const prop of type.getProperties()) {
    const propName = prop.getName();
    // Skip Symbol-keyed properties (tsc internal names like __@iterator@35)
    if (isSymbolProperty(propName)) continue;

    const propType = checker.getTypeOfSymbol(prop);
    const propDecl = prop.getDeclarations()?.[0];

    const callSignatures = propType.getCallSignatures();
    if (callSignatures.length > 0) {
      methods[propName] = {
        signatures: callSignatures.map((sig) =>
          extractSignature(sig, checker),
        ),
      };
    } else {
      // Skip private/protected members
      if (propDecl && hasPrivateModifier(propDecl)) continue;

      const isOptional = !!(prop.flags & ts.SymbolFlags.Optional);
      const isReadonly = propDecl ? hasReadonlyModifier(propDecl) : false;
      fields.push({
        name: propName,
        type: convertType(propType, checker),
        ...(isOptional ? { optional: true } : {}),
        ...(isReadonly ? { readonly: true } : {}),
      });
    }
  }

  // Constructor
  const classType = checker.getTypeOfSymbol(type.symbol);
  for (const sig of classType.getConstructSignatures()) {
    constructors.push(extractSignature(sig, checker));
  }

  // Extract type parameters
  const typeParams = extractTypeParams(node.typeParameters, checker);

  return {
    kind: "interface",
    ...(typeParams.length > 0 ? { type_params: typeParams } : {}),
    fields,
    methods,
    constructors,
  };
}

function extractFunction(
  node: ts.FunctionDeclaration,
  checker: ts.TypeChecker,
): ExternalTypeDef {
  const type = checker.getTypeAtLocation(node);
  const signatures = type
    .getCallSignatures()
    .map((sig) => extractSignature(sig, checker));
  return { kind: "function", signatures };
}

function extractTypeAlias(
  type: ts.Type,
  checker: ts.TypeChecker,
): ExternalTypeDef {
  return { kind: "alias", type: convertType(type, checker) };
}

function extractSignature(
  sig: ts.Signature,
  checker: ts.TypeChecker,
): ExternalSignature {
  const params: ExternalParam[] = sig.parameters.map((param) => {
    const paramType = checker.getTypeOfSymbol(param);
    const paramDecl = param.getDeclarations()?.[0];
    const isOptional = !!(param.flags & ts.SymbolFlags.Optional);
    const isRest =
      paramDecl !== undefined &&
      ts.isParameter(paramDecl) &&
      paramDecl.dotDotDotToken !== undefined;
    return {
      name: param.getName(),
      type: convertType(paramType, checker),
      ...(isOptional ? { optional: true } : {}),
      ...(isRest ? { rest: true } : {}),
    };
  });

  const returnType = sig.getReturnType();

  // Extract signature-level type parameters from the AST declaration.
  // Used for method-level generics like `then<TResult1, TResult2>(...)` so that
  // the Rust loader can push them into the synthetic registry's type_param scope
  // when walking parameter / return types (I-383 T2.A-i).
  const sigDecl = sig.getDeclaration() as
    | (ts.SignatureDeclaration & {
        typeParameters?: ts.NodeArray<ts.TypeParameterDeclaration>;
      })
    | undefined;
  const typeParams = extractTypeParams(sigDecl?.typeParameters, checker);

  return {
    ...(typeParams.length > 0 ? { type_params: typeParams } : {}),
    params,
    return_type: convertType(returnType, checker),
  };
}

function mergeConstructors(
  existing: ExternalInterfaceDef,
  typeNode: ts.TypeNode,
  checker: ts.TypeChecker,
): void {
  const type = checker.getTypeAtLocation(typeNode);
  for (const sig of type.getConstructSignatures()) {
    existing.constructors.push(extractSignature(sig, checker));
  }
}

/** Extracts type parameters from a TypeParameterDeclaration (AST node). */
function extractTypeParams(
  typeParameters: ts.NodeArray<ts.TypeParameterDeclaration> | undefined,
  checker: ts.TypeChecker,
): ExternalTypeParam[] {
  if (!typeParameters) return [];
  return typeParameters.map((tp) => ({
    name: tp.name.text,
    ...(tp.constraint
      ? { constraint: convertType(checker.getTypeAtLocation(tp.constraint), checker) }
      : {}),
  }));
}

// ── Type conversion ────────────────────────────────────────────────

/** Maximum recursion depth to prevent infinite loops on circular types. */
const MAX_DEPTH = 10;

function convertType(
  type: ts.Type,
  checker: ts.TypeChecker,
  depth: number = 0,
): ExternalType {
  if (depth > MAX_DEPTH) {
    return { kind: "any" };
  }

  const typeStr = checker.typeToString(type);

  // Primitive types
  if (type.flags & ts.TypeFlags.String) return { kind: "string" };
  if (type.flags & ts.TypeFlags.Number) return { kind: "number" };
  if (type.flags & ts.TypeFlags.Boolean) return { kind: "boolean" };
  if (type.flags & ts.TypeFlags.Void) return { kind: "void" };
  if (type.flags & ts.TypeFlags.Undefined) return { kind: "undefined" };
  if (type.flags & ts.TypeFlags.Null) return { kind: "null" };
  if (type.flags & ts.TypeFlags.Never) return { kind: "never" };
  if (type.flags & ts.TypeFlags.Any) return { kind: "any" };
  if (type.flags & ts.TypeFlags.Unknown) return { kind: "unknown" };

  // String/number literal types → treat as their base type
  if (type.flags & ts.TypeFlags.StringLiteral) return { kind: "string" };
  if (type.flags & ts.TypeFlags.NumberLiteral) return { kind: "number" };
  if (type.flags & ts.TypeFlags.BooleanLiteral) return { kind: "boolean" };

  // Union type
  if (type.isUnion()) {
    const members = type.types.map((t) => convertType(t, checker, depth + 1));
    // Deduplicate (e.g., boolean = true | false)
    const seen = new Set<string>();
    const unique = members.filter((m) => {
      const key = JSON.stringify(m);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    if (unique.length === 1) return unique[0];
    return { kind: "union", members: unique };
  }

  // Intersection type
  //
  // I-383 T2.A-i: 旧実装は `{ kind: "named", name: typeStr }` で raw 文字列を返していたが、
  // これは Rust loader 側で `ArrayBuffer & { BYTES_PER_ELEMENT?: never; }` のような
  // 不正な named 型として外部参照に leak する silent semantic defect。intersection を
  // ExternalType に構造化するのは ExternalType schema 拡張 + Rust loader での struct
  // merge 実装が必要で T2.A-i のスコープ外なので、ここでは安全な `any` fallback に
  // 退避する (型推論の constraint 弱化に留まり、後段の構造を破壊しない)。
  // 構造化対応は別 PRD で行う (TODO 起票候補)。
  if (type.isIntersection()) {
    return { kind: "any" };
  }

  // Tuple type
  if (checker.isTupleType(type)) {
    const typeRef = type as ts.TypeReference;
    const typeArgs = checker.getTypeArguments(typeRef);
    return {
      kind: "tuple",
      elements: typeArgs.map((t) => convertType(t, checker, depth + 1)),
    };
  }

  // Array type
  if (checker.isArrayType(type)) {
    const typeRef = type as ts.TypeReference;
    const typeArgs = checker.getTypeArguments(typeRef);
    if (typeArgs.length > 0) {
      return {
        kind: "array",
        element: convertType(typeArgs[0], checker, depth + 1),
      };
    }
    return { kind: "array", element: { kind: "any" } };
  }

  // Function type (has call signatures but no properties → pure function)
  const callSignatures = type.getCallSignatures();
  if (
    callSignatures.length > 0 &&
    type.getProperties().length === 0
  ) {
    const sig = callSignatures[0];
    const params = sig.parameters.map((p) =>
      convertType(checker.getTypeOfSymbol(p), checker, depth + 1),
    );
    const returnType = convertType(
      sig.getReturnType(),
      checker,
      depth + 1,
    );
    return { kind: "function", params, return_type: returnType };
  }

  // Anonymous type literal (tsc internal symbol "__type").
  //
  // TypeScript assigns the internal symbol name "__type" to anonymous object type
  // literals that appear inline as field types or parameter types, e.g.:
  //   interface Foo { handler: { (s: string): boolean } }
  //   interface Bar { groups?: { [key: string]: string } }
  //   interface Baz { match(m: { [Symbol.match](s: string): T }): T }
  //
  // The symbol "__type" is a compiler internal and must not leak into the JSON output.
  // Instead, expand the anonymous type based on its actual structure:
  //   1. Has call signatures → function type
  //   2. Has index signatures → Record<K, V>
  //   3. Has Symbol-keyed properties with call signatures → function type (from property)
  //   4. Otherwise → any (truly opaque anonymous type)
  {
    const sym = type.symbol ?? type.aliasSymbol;
    if (sym && sym.name === "__type") {
      // 1. Direct call signatures → function type
      if (callSignatures.length > 0) {
        const sig = callSignatures[0];
        const params = sig.parameters.map((p) =>
          convertType(checker.getTypeOfSymbol(p), checker, depth + 1),
        );
        const returnType = convertType(
          sig.getReturnType(),
          checker,
          depth + 1,
        );
        return { kind: "function", params, return_type: returnType };
      }

      // 2. Index signatures → Record<K, V>
      const indexInfos = checker.getIndexInfosOfType(type);
      if (indexInfos.length > 0) {
        const idx = indexInfos[0];
        const keyType = convertType(idx.keyType, checker, depth + 1);
        const valueType = convertType(idx.type, checker, depth + 1);
        return { kind: "named", name: "Record", type_args: [keyType, valueType] };
      }

      // 3. Symbol-keyed properties with call signatures → function type
      //    e.g., { [Symbol.match](s: string): RegExpMatchArray | null }
      const props = type.getProperties();
      if (props.length > 0 && props.every((p) => isSymbolProperty(p.getName()))) {
        const firstProp = props[0];
        const propType = checker.getTypeOfSymbol(firstProp);
        const propCallSigs = propType.getCallSignatures();
        if (propCallSigs.length > 0) {
          const sig = propCallSigs[0];
          const params = sig.parameters.map((p) =>
            convertType(checker.getTypeOfSymbol(p), checker, depth + 1),
          );
          const returnType = convertType(
            sig.getReturnType(),
            checker,
            depth + 1,
          );
          return { kind: "function", params, return_type: returnType };
        }
      }

      // 4. Truly opaque anonymous type
      return { kind: "any" };
    }
  }

  // Named type with type arguments (e.g., Promise<Response>)
  if ((type as ts.TypeReference).typeArguments) {
    const typeRef = type as ts.TypeReference;
    const symbol = type.symbol ?? type.aliasSymbol;
    const name = symbol
      ? checker.symbolToString(symbol)
      : typeStr;
    const typeArgs = (typeRef.typeArguments ?? []).map((t) =>
      convertType(t, checker, depth + 1),
    );
    return { kind: "named", name, type_args: typeArgs };
  }

  // Named type without type arguments
  const symbol = type.symbol ?? type.aliasSymbol;
  if (symbol) {
    return { kind: "named", name: checker.symbolToString(symbol) };
  }

  // Fallback: use typeToString
  return { kind: "named", name: typeStr };
}

// ── Helpers ────────────────────────────────────────────────────────

/** Strips `undefined` from a union type (e.g., `string | undefined` → `string`). */
function stripUndefined(type: ts.Type, checker: ts.TypeChecker): ts.Type {
  if (type.isUnion()) {
    const filtered = type.types.filter(
      (t) => !(t.flags & ts.TypeFlags.Undefined),
    );
    if (filtered.length === 0) return type;
    if (filtered.length === 1) return filtered[0];
    // Multiple non-undefined members remain — return original type.
    // The union will be serialized as-is without the undefined member
    // during convertType since we only strip at the field level.
    return type;
  }
  return type;
}

/** Returns true if the property name is a Symbol-keyed property (tsc internal format: `__@name@NNN`). */
function isSymbolProperty(name: string): boolean {
  return name.startsWith("__@");
}

function hasReadonlyModifier(node: ts.Node): boolean {
  const modifiers = ts.canHaveModifiers(node)
    ? ts.getModifiers(node)
    : undefined;
  return modifiers?.some((m) => m.kind === ts.SyntaxKind.ReadonlyKeyword) ?? false;
}

function hasPrivateModifier(node: ts.Node): boolean {
  const modifiers = ts.canHaveModifiers(node)
    ? ts.getModifiers(node)
    : undefined;
  return (
    modifiers?.some(
      (m) =>
        m.kind === ts.SyntaxKind.PrivateKeyword ||
        m.kind === ts.SyntaxKind.ProtectedKeyword,
    ) ?? false
  );
}
