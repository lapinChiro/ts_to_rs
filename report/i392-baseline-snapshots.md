# I-392 Baseline Snapshots (P0.0)

## Baseline Metrics

| Check | Result |
|---|---|
| `cargo check` | pass |
| `cargo test --lib` | 2295 pass, 0 fail |
| `cargo clippy` | 0 warning |
| `cargo fmt --check` | 0 diff |
| `cargo llvm-cov` | 91.63% (threshold 89%) |
| Hono bench | clean=71.5%, errors=58 |

## 既存 Fixture

### `tests/fixtures/callable-interface.input.ts` (30 行)

```typescript
interface GetValue {
    (key: string): string;
}

interface GetCookie {
    (c: string): string;
    (c: string, key: string): number;
}

interface Factory {
    new (config: string): Factory;
    name: string;
}

interface Body {
    text: string;
    json: boolean;
}

type BodyCache = Body;

const getValue: GetValue = (key: string): string => {
    return key;
};
```

### 既存 Snapshot (`integration_test__callable_interface.snap`)

```rust
type GetValue = Box<dyn Fn(String) -> String>;

type GetCookie = Box<dyn Fn(String, String) -> f64>;

#[derive(Debug, Clone, PartialEq)]
struct Body {
    text: String,
    json: bool,
}

type BodyCache = Body;

fn getValue(key: String) -> String {
    key
}
```

**問題点**:
- `GetCookie` の overload 1 (`(c: string): string`) の return type が消失
- `getValue` が free function に変換され、`GetValue` trait との関連が失われる

## Snapshot 影響リスト (Step 6b)

P4.1 trait 化で影響を受ける callable interface 型を含む snapshot:

| Snapshot file | Line(s) | 現在の出力 | 備考 |
|---|---|---|---|
| `integration_test__callable_interface.snap` | L5 | `type GetValue = Box<dyn Fn(String) -> String>` | 主要 fixture |
| `integration_test__callable_interface.snap` | L7 | `type GetCookie = Box<dyn Fn(String, String) -> f64>` | 主要 fixture |
| `integration_test__var_type_alias_arrow.snap` | L15 | `type GetConnInfo = Box<dyn Fn(String) -> ConnInfo>` | single overload |
| `integration_test__interface_mixed.snap` | L5 | `pub type Handler = Box<dyn Fn(String) -> f64>` | single overload |
| `integration_test__call_signature_rest.snap` | L5 | `type VarargHandler = Box<dyn Fn(Vec<f64>)>` | rest params |
| `integration_test__call_signature_rest.snap` | L7 | `type Formatter = Box<dyn Fn(String, Vec<f64>) -> String>` | rest params |
| `integration_test__call_signature_rest.snap` | L9 | `type Callback = Box<dyn Fn(f64, f64) -> f64>` | 通常 callable |

**影響なし** (callable interface ではなく関数型パラメータ/戻り値):
- `closures.snap`, `functions.snap`, `void_type.snap`, `union_fallback.snap`
