// I-379 E2E: TS `null` / `undefined` 値の構造化変換が runtime semantics を
// 維持することを stdout 比較で検証する。
//
// I-379 は `Expr::Ident("None")` 文字列 encoding を `Expr::BuiltinVariantValue(None)`
// に置換した。display 表現は両者とも `None` のため byte-for-byte 同一だが、構造化
// により walker / fold / generator の意味論パスが変わる。本 E2E は意味論的等価性
// を runtime 観察で lock-in する。
//
// 直接 `console.log(null)` を比較すると TS は `"null"` / Rust は `"None"` を
// print して不一致になるため、null/undefined を直接出力せず派生 boolean / number
// を出力する。

function isNullValue(x: string | null): boolean {
    return x === null;
}

function isUndefinedValue(x: number | undefined): boolean {
    return x === undefined;
}

function lengthOrZero(s: string | null): number {
    if (s === null) {
        return 0;
    }
    return s.length;
}

function valueOrFallback(v: number | undefined, fallback: number): number {
    return v ?? fallback;
}

function main(): void {
    // null literal in Option context (production site #1: literals.rs:48)
    console.log("isNull(null):", isNullValue(null));
    console.log("isNull(\"hi\"):", isNullValue("hi"));

    // undefined identifier in Option context (production site #3: mod.rs:95)
    console.log("isUndef(undef):", isUndefinedValue(undefined));
    console.log("isUndef(42):", isUndefinedValue(42));

    // null narrowing branch with len
    console.log("lenOrZero(null):", lengthOrZero(null));
    console.log("lenOrZero(\"hello\"):", lengthOrZero("hello"));

    // nullish coalescing with undefined (production site #2: mod.rs:58)
    console.log("valueOr(undef,99):", valueOrFallback(undefined, 99));
    console.log("valueOr(7,99):", valueOrFallback(7, 99));
}
