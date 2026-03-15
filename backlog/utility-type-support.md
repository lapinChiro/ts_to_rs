# ユーティリティ型の TypeRegistry 連携（Partial/Required/Pick/Omit/NonNullable）

## 背景・動機

TS の組み込みユーティリティ型（`Partial<T>`, `Required<T>`, `Pick<T, K>`, `Omit<T, K>`, `NonNullable<T>`）が未対応で、型注記位置でそのまま `Named` 型として通過してしまう。これらは TS で頻出し、Hono を含む実プロジェクトで広く使われる。

I-41（TypeRegistry 伝搬）が完了し、`convert_type_ref` が `&TypeRegistry` を受け取るようになったため、着手可能になった。

## ゴール

- `Partial<T>` → TypeRegistry 登録済みなら全フィールドを `Option<T>` にした合成 struct を生成
- `Required<T>` → TypeRegistry 登録済みなら全フィールドから `Option` を剥がした合成 struct を生成
- `Pick<T, K>` → TypeRegistry 登録済みなら指定フィールドのみの合成 struct を生成
- `Omit<T, K>` → TypeRegistry 登録済みなら指定フィールド以外の合成 struct を生成
- `NonNullable<T>` → `Option<T>` から `T` を取り出す（`Option` の剥がし）
- 合成 struct の命名が変換内容を反映する（`PartialPoint`, `PickPointXY` 等）
- ネストしたユーティリティ型（`Partial<Pick<T, "a" | "b">>>`）が再帰的に処理される
- TypeRegistry 未登録時は inner type をそのまま返す（グレースフルフォールバック）
- 全テスト pass、clippy 0 警告、fmt 通過

## スコープ

### 対象

- `convert_type_ref` に `Partial`, `Required`, `Pick`, `Omit`, `NonNullable` の分岐を追加
- TypeRegistry からフィールド一覧を取得し、加工した合成 struct を `extra_items` に追加
- 合成 struct の意味的命名（`Partial{TypeName}`, `Pick{TypeName}{Fields}` 等）
- ネスト対応（再帰的に inner type を処理してから加工）
- TypeRegistry 未登録時のフォールバック

### 対象外

- `Exclude<T, U>` / `Extract<T, U>` — union 型操作で、struct フィールド加工とは異なるアプローチが必要
- `ReturnType<T>` / `Parameters<T>` — 関数型操作

## 設計

### 技術的アプローチ

#### convert_type_ref の拡張

```rust
"Partial" => convert_utility_partial(params, extra_items, reg),
"Required" => convert_utility_required(params, extra_items, reg),
"Pick" => convert_utility_pick(params, extra_items, reg),
"Omit" => convert_utility_omit(params, extra_items, reg),
"NonNullable" => convert_utility_non_nullable(params, extra_items, reg),
```

#### Partial<T> の処理

1. `T` の型名を取得
2. TypeRegistry から `T` のフィールド一覧を取得
3. 全フィールドの型を `Option<ty>` でラップ（既に Option の場合はそのまま）
4. `Partial{TypeName}` という名前で合成 struct を `extra_items` に追加
5. `RustType::Named { name: "Partial{TypeName}" }` を返す
6. TypeRegistry 未登録の場合: inner type をそのまま返す

#### Pick<T, K> の処理

1. `T` の型名を取得
2. `K` が string literal union の場合、キー名のリストを抽出
3. TypeRegistry から `T` のフィールドのうち、キーリストに含まれるものだけ抽出
4. 合成 struct を生成

#### 命名規則

| 入力 | 合成名 |
|------|--------|
| `Partial<Point>` | `PartialPoint` |
| `Required<Config>` | `RequiredConfig` |
| `Pick<Point, "x" \| "y">` | `PickPointXY` |
| `Omit<Config, "debug">` | `OmitConfigDebug` |

#### ネスト対応

`Partial<Pick<Point, "x" | "y">>` の場合:
1. まず inner の `Pick<Point, "x" | "y">` を再帰処理 → `PickPointXY` を生成
2. `PickPointXY` を TypeRegistry には登録しないが、`extra_items` に追加された struct のフィールド情報を使って Partial を適用
3. → `PartialPickPointXY` を生成

ネスト対応には、合成 struct のフィールド情報を `extra_items` から逆引きする必要がある。`extra_items` を走査して直前に追加された struct のフィールドを取得する。

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_type_ref` にユーティリティ型分岐追加、ヘルパー関数群追加
- テストファイル

## 作業ステップ

### Part A: Partial / Required

- [ ] ステップ1（RED）: `Partial<Point>` のテスト（TypeRegistry 登録済み → 全フィールド Option の合成 struct）
- [ ] ステップ2（GREEN）: `convert_utility_partial` 実装
- [ ] ステップ3（RED）: `Required<OptionalConfig>` のテスト（Option 剥がし）
- [ ] ステップ4（GREEN）: `convert_utility_required` 実装

### Part B: Pick / Omit

- [ ] ステップ5（RED）: `Pick<Point, "x">` のテスト
- [ ] ステップ6（GREEN）: `convert_utility_pick` 実装
- [ ] ステップ7（RED）: `Omit<Point, "x">` のテスト
- [ ] ステップ8（GREEN）: `convert_utility_omit` 実装

### Part C: NonNullable + ネスト

- [ ] ステップ9（RED）: `NonNullable<string | null>` のテスト
- [ ] ステップ10（GREEN）: `convert_utility_non_nullable` 実装
- [ ] ステップ11（RED）: `Partial<Pick<Point, "x" | "y">>` のネストテスト
- [ ] ステップ12（GREEN）: ネスト対応実装

### Part D: 統合

- [ ] ステップ13: 未登録型フォールバック確認 + 回帰テスト
- [ ] ステップ14: Quality check

## テスト計画

### Partial / Required

- `Partial<Point>` (登録済み: x: f64, y: f64) → `PartialPoint { x: Option<f64>, y: Option<f64> }`
- `Partial<Unknown>` (未登録) → `Unknown` そのまま
- `Required<OptPoint>` (登録済み: x: Option<f64>, y: Option<f64>) → `RequiredOptPoint { x: f64, y: f64 }`

### Pick / Omit

- `Pick<Point, "x">` (登録済み) → `PickPointX { x: f64 }`
- `Pick<Point, "x" | "y">` → `PickPointXY { x: f64, y: f64 }`
- `Omit<Point, "x">` (登録済み: x, y, z) → `OmitPointX { y: f64, z: f64 }`

### NonNullable

- `NonNullable<string | null>` → `String`（union から null を除去）
- `NonNullable<Option<string>>` → `String`（Option 剥がし）

### ネスト

- `Partial<Pick<Point, "x" | "y">>` → `PartialPickPointXY { x: Option<f64>, y: Option<f64> }`

## 完了条件

- 5 つのユーティリティ型が TypeRegistry 連携で正しく変換される
- 合成 struct 名が変換内容を反映する
- ネストしたユーティリティ型が再帰的に処理される
- TypeRegistry 未登録時にグレースフルフォールバックする
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
