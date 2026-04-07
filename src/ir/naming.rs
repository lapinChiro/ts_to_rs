//! 識別子変換ルール（sanitize / case 変換）。
//!
//! TS の識別子・文字列を Rust の有効な識別子に変換するルールを単一定義点として集約する。

/// struct フィールド名を有効な Rust 識別子文字列に変換する（文字レベルのサニタイズ）。
///
/// TypeScript のオブジェクトキーは任意の文字列だが、Rust の識別子には制約がある。
/// 以下の変換を適用:
/// 1. ハイフン → アンダースコア（`Content-Type` → `Content_Type`）
/// 2. ブラケット除去（`foo[]` → `foo`）
/// 3. `_` のみ → `_field`（Rust では `_` は破棄パターン）
/// 4. 先頭が数字 → `_` プレフィクス
/// 5. 空文字列 → `_empty`
///
/// 注意: Rust 予約語のエスケープ（`r#` プレフィクス）は行わない。
/// それは generator の `escape_ident` の責務。
pub fn sanitize_field_name(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '-' => sanitized.push('_'),
            '[' | ']' => {}
            _ => sanitized.push(ch),
        }
    }

    if sanitized == "_" {
        return "_field".to_string();
    }

    if sanitized.starts_with(|c: char| c.is_ascii_digit()) {
        sanitized.insert(0, '_');
    }

    if sanitized.is_empty() {
        return "_empty".to_string();
    }

    sanitized
}

/// Converts a string value to PascalCase for use as an enum variant name.
///
/// Examples: `"up"` → `"Up"`, `"foo-bar"` → `"FooBar"`, `"UPPER_CASE"` → `"UpperCase"`
pub fn string_to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Rust prelude type names that would cause shadowing if used as user-defined type names.
///
/// Includes types, enum variants, and common std types that are in the prelude or
/// automatically imported. Using these as enum/struct names would shadow the standard
/// library definitions, causing compile errors or silent semantic changes.
const RUST_PRELUDE_TYPE_NAMES: &[&str] = &[
    // Core prelude types
    "Option", "Result", "String", "Vec", "Box",
    // Core prelude enum variants (used as value constructors)
    "Some", "None", "Ok", "Err", // Special keyword
    "Self",
];

/// Sanitizes a type name to avoid shadowing Rust prelude types.
///
/// If `name` matches a Rust prelude type name, prefixes it with "Ts"
/// (e.g., `Result` → `TsResult`). Otherwise returns the name unchanged.
pub fn sanitize_rust_type_name(name: &str) -> String {
    if RUST_PRELUDE_TYPE_NAMES.contains(&name) {
        format!("Ts{name}")
    } else {
        name.to_string()
    }
}

/// camelCase を snake_case に変換する。
///
/// 連続する大文字は略語として扱い、最後の大文字を次の単語の先頭とする。
/// 例: `"byteLength"` → `"byte_length"`, `"toISOString"` → `"to_iso_string"`
pub fn camel_to_snake(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                let prev_upper = chars[i - 1].is_uppercase();
                let next_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
                if !prev_upper || next_lower {
                    result.push('_');
                }
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
}
