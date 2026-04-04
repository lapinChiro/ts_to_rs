// Type narrowing: if-let pattern generation for null check and typeof on union

// Case 1: null check → if let Some(x) = x
function processOptional(x: string | null): void {
    if (x !== null) {
        console.log(x);
    }
}

// Case 2: typeof "string" on union type → if let Enum::String(x) = x
function processUnion(x: string | number): string {
    if (typeof x === "string") {
        return x.trim();
    }
    return "not a string";
}

// Case 3: typeof "object" on any → enum with Object variant + if let
function processObject(x: any): void {
    if (typeof x === "object") {
        console.log(x);
    }
}

// Case 4: compound && with two typeof guards → nested if let
function processCompound(x: string | number, y: string | number): void {
    if (typeof x === "string" && typeof y === "number") {
        console.log(x.length);
        console.log(y.toFixed(2));
    }
}

// Case 5: compound && with typeof + null check → nested if let
function processMixed(x: string | number, y: string | null): void {
    if (typeof x === "string" && y !== null) {
        console.log(x.length);
        console.log(y);
    }
}

// Case 6: compound && with guard + non-guard condition → if let + nested if
function processGuardAndCondition(x: string | number): void {
    if (typeof x === "string" && x.length > 0) {
        console.log(x);
    }
}

// Case 7: compound && with else branch → else duplicated at all nesting levels
function processCompoundWithElse(x: string | number, y: string | number): string {
    if (typeof x === "string" && typeof y === "number") {
        return x.trim();
    } else {
        return "fallback";
    }
}

// Case 8: guard + non-guard + else → else at both if-let and inner if levels
function processGuardConditionElse(x: string | number): string {
    if (typeof x === "string" && x.length > 0) {
        return x;
    } else {
        return "empty or not string";
    }
}

// Case 9: !== guard in compound → branch swap interacts correctly with nesting
function processNeqCompound(x: string | number, y: string | null): void {
    if (typeof x !== "number" && y !== null) {
        console.log(x);
        console.log(y);
    } else {
        console.log("fallback");
    }
}

// Case 10: ternary with typeof → if let narrowing in then branch
function ternaryTypeof(x: string | number): number {
    return typeof x === "string" ? x.length : 0;
}

// Case 11: ternary with null check → if let Some narrowing
function ternaryNullCheck(x: string | null): string {
    return x !== null ? x.trim() : "default";
}

// Case 12: ternary without narrowing guard → unchanged (existing behavior)
function ternaryNoGuard(a: number): string {
    return a > 0 ? "positive" : "non-positive";
}

// Case 13: !== ternary → swap + narrowing on the else (alt) branch
function ternaryNeqTypeof(x: string | number): number {
    return typeof x !== "string" ? 0 : x.length;
}

// Case 14: switch (typeof x) on union → match with enum variants
function switchTypeof(x: string | number): string {
    switch (typeof x) {
        case "string":
            return x.trim();
        case "number":
            return x.toFixed(2);
        default:
            return "unknown";
    }
}

// Case 15: switch (typeof x) with narrowing in case body
function switchTypeofNarrow(x: string | number): number {
    switch (typeof x) {
        case "string":
            return x.length;
        default:
            return 0;
    }
}

// Case 16: switch (typeof x) with fall-through (empty body cases)
function switchTypeofFallthrough(x: string | number): string {
    switch (typeof x) {
        case "string":
        case "number":
            return "primitive";
        default:
            return "other";
    }
}

// Case 17: === null with explicit else → if let swap (I-327)
function eqNullWithElse(x: string | null): string {
    if (x === null) {
        return "null";
    } else {
        return x.trim();
    }
}

// Case 18: === undefined with explicit else → if let swap
function eqUndefinedWithElse(x: string | undefined): string {
    if (x === undefined) {
        return "undefined";
    } else {
        return x.trim();
    }
}

// Case 19: typeof !== with else → narrowing in else branch
function typeofNeqWithElse(x: string | number): string {
    if (typeof x !== "string") {
        return "not string";
    } else {
        return x.trim();
    }
}

// Case 20: unknown typeof narrowing (I-333)
function unknownTypeof(x: unknown): string {
    if (typeof x === "string") {
        return x.toUpperCase();
    }
    return "not a string";
}

// Case 21: complement narrowing in else (2-variant, I-213)
function complementElse2(x: string | number): string {
    if (typeof x === "string") {
        return x.trim();
    } else {
        return x.toFixed(2);
    }
}

// Case 22: early return narrowing (I-213)
function earlyReturnNarrowing(x: string | number): number {
    if (typeof x === "string") {
        return x.length;
    }
    return x;
}

// Case 23: early return with null check (I-213)
function earlyReturnNull(x: string | null): string {
    if (x === null) {
        return "null";
    }
    return x.trim();
}

// Case 24: compound ternary (I-214)
function compoundTernary(x: string | number, y: string | null): number {
    return typeof x === "string" && y !== null ? x.length : 0;
}
