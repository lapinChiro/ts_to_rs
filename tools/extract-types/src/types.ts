/**
 * JSON interchange format types for communication between
 * the tsc type extraction script and the Rust loader.
 *
 * These types define the structure of the JSON output.
 * Version must match FORMAT_VERSION (= 2) in src/external_types.rs.
 */

/** Top-level output structure. */
export interface ExternalTypesJson {
  version: 2;
  types: Record<string, ExternalTypeDef>;
}

/** A single type definition. */
export type ExternalTypeDef =
  | ExternalInterfaceDef
  | ExternalFunctionDef
  | ExternalAliasDef;

export interface ExternalTypeParam {
  name: string;
  constraint?: ExternalType;
}

export interface ExternalInterfaceDef {
  kind: "interface";
  type_params?: ExternalTypeParam[];
  fields: ExternalField[];
  methods: Record<string, ExternalMethod>;
  constructors: ExternalSignature[];
}

export interface ExternalFunctionDef {
  kind: "function";
  signatures: ExternalSignature[];
}

export interface ExternalAliasDef {
  kind: "alias";
  type: ExternalType;
}

export interface ExternalField {
  name: string;
  type: ExternalType;
  optional?: boolean;
  readonly?: boolean;
}

export interface ExternalMethod {
  signatures: ExternalSignature[];
}

export interface ExternalSignature {
  type_params?: ExternalTypeParam[];
  params: ExternalParam[];
  return_type?: ExternalType;
}

export interface ExternalParam {
  name: string;
  type: ExternalType;
  optional?: boolean;
  rest?: boolean;
}

/** TypeScript type representation. */
export type ExternalType =
  | { kind: "string" }
  | { kind: "number" }
  | { kind: "boolean" }
  | { kind: "void" }
  | { kind: "any" }
  | { kind: "unknown" }
  | { kind: "never" }
  | { kind: "null" }
  | { kind: "undefined" }
  | { kind: "named"; name: string; type_args?: ExternalType[] }
  | { kind: "array"; element: ExternalType }
  | { kind: "tuple"; elements: ExternalType[] }
  | { kind: "union"; members: ExternalType[] }
  | { kind: "function"; params: ExternalType[]; return_type: ExternalType };
