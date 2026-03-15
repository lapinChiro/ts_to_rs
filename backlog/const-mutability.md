# const のミュータビリティ差異の対応

## 背景・動機

TS の `const` はオブジェクトの再代入を禁止するが、フィールドの変更は許可する。Rust の `let` はフィールド変更も禁止する。`const obj = {}; obj.x = 1;` は TS で有効だが、変換後の Rust ではコンパイル不可。

関連コード: `src/transformer/statements/mod.rs:95` — `const` → `let`（immutable）。

## ゴール

`const` で宣言されたオブジェクトのフィールドが変更される場合、`let mut` として生成される。

## スコープ

### 対象

- `const` 宣言の本体内でフィールド代入（`obj.x = val`）があるか検出
- フィールド代入がある場合は `let mut` に変更

### 対象外

- `Cell`/`RefCell` による interior mutability
- ネストしたオブジェクトの deep mutability 検出

## 設計

### 技術的アプローチ

関数本体の後続文をスキャンして、変数名への `FieldAccess` 代入があるか検出する。

ただしこの検出は `convert_stmt` のスコープでは困難（後続文が見えない）。代替案:

**案 A**: `convert_stmt_list` で 2 パスを行う（1 パス目で変数ごとのフィールド代入を検出、2 パス目で const の mutability を決定）
**案 B**: 初版は const を常に `let mut` にする（Rust では `let mut` でもフィールド変更しなければ警告のみ）
**案 C**: const + オブジェクト型の組み合わせを `let mut` にする

推奨: **案 C**。型情報が利用可能であり、オブジェクト型（Named 型、struct 参照）の `const` は `let mut` にする。プリミティブ型の `const` は `let` のまま。

### 影響範囲

- `src/transformer/statements/mod.rs` — `convert_var_decl`
- テストファイル

## 作業ステップ

- [ ] ステップ1（RED）: `const obj = { x: 1 }; obj.x = 2;` がコンパイル可能なテスト追加
- [ ] ステップ2（GREEN）: const + オブジェクト型 → `let mut` の変換
- [ ] ステップ3: Quality check

## テスト計画

- `const obj: Foo = ...` → `let mut obj: Foo = ...`
- `const x: number = 1` → `let x: f64 = 1.0`（プリミティブは immutable 維持）
- 回帰: 既存の const テスト

## 完了条件

- オブジェクト型の `const` が `let mut` で生成される
- プリミティブ型の `const` が `let` のまま
- 全テスト pass、0 errors / 0 warnings
