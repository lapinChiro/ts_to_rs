# クラス継承変換

## 背景・動機

TS の `class Child extends Parent` は頻出パターンだが、Rust にはクラス継承がない。trait + struct で表現することで、親クラスのメソッドを子クラスで再利用する TS のパターンを Rust に変換できる。

## ゴール

TS の `extends` によるクラス継承を、Rust の trait + struct + impl で変換できる。

### 変換例

```typescript
class Animal {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    speak(): string {
        return this.name;
    }
}

class Dog extends Animal {
    constructor(name: string) {
        super(name);
    }
    bark(): string {
        return this.speak() + " barks";
    }
}
```
→
```rust
pub struct Animal {
    pub name: String,
}

pub trait AnimalTrait {
    fn name(&self) -> &String;
    fn speak(&self) -> String;
}

impl Animal {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl AnimalTrait for Animal {
    fn name(&self) -> &String {
        &self.name
    }
    fn speak(&self) -> String {
        self.name.clone()
    }
}

pub struct Dog {
    pub name: String,
}

impl Dog {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn bark(&self) -> String {
        format!("{} barks", self.speak())
    }
}

impl AnimalTrait for Dog {
    fn name(&self) -> &String {
        &self.name
    }
    fn speak(&self) -> String {
        self.name.clone()
    }
}
```

## スコープ

### 対象

- `class Child extends Parent` → 親メソッドの trait 化 + 子での impl
- `super()` コンストラクタ呼び出し → 親フィールドの初期化
- 親クラスのフィールドを子クラスにコピー

### 対象外

- `implements`（interface の実装、trait 化は別タスク）
- `abstract class`（抽象クラス）
- `protected` メンバー（Rust に直接対応なし）

## 設計

### 技術的アプローチ

1. **IR 拡張**: `Item` に `Trait` バリアントを追加（trait 名、メソッドシグネチャのリスト）。`Item::Impl` に trait 実装を表現するフィールドを追加
2. **transformer 変更**: `extends` 付きクラスを検出したとき、親クラスから trait を生成し、子クラスに trait impl を追加。親のフィールドを子にコピー
3. **generator 更新**: `Item::Trait` の出力、trait impl ブロック（`impl TraitName for StructName`）の出力

### 影響範囲

- `src/ir.rs` — `Item::Trait` 追加、`Item::Impl` に trait 実装の情報追加
- `src/transformer/classes.rs` — `extends` の検出と trait 生成ロジック
- `src/transformer/mod.rs` — 親子クラスの関係を解決する順序制御
- `src/generator.rs` — `Item::Trait`、trait impl ブロックの生成
- `tests/fixtures/` — クラス継承用の fixture 追加

## 作業ステップ

- [ ] ステップ1: IR 拡張 — `Item::Trait` を追加。`Item::Impl` に `for_trait: Option<String>` を追加
- [ ] ステップ2: transformer — `extends` 付きクラスから親の trait 定義を生成
- [ ] ステップ3: transformer — 子クラスに親フィールドをコピーし、trait impl を生成
- [ ] ステップ4: transformer — `super()` を親フィールドの初期化に変換
- [ ] ステップ5: generator — `trait` 定義と `impl TraitName for StructName` ブロックの出力
- [ ] ステップ6: スナップショットテスト — fixture ファイルで E2E 検証

## テスト計画

- 正常系: 単純な 1 段階の継承、親にフィールドとメソッドがあるケース
- 正常系: 子クラスが独自メソッドを持つケース
- 正常系: `super()` に引数を渡すケース
- 異常系: 親クラスが同一ファイルに存在しない場合（エラーまたは警告）
- 境界値: 親にメソッドがない場合（trait が空）、親にフィールドがない場合
- スナップショット: `tests/fixtures/class-inheritance.input.ts` で E2E 検証

## 完了条件

- 上記変換例が正しく変換される
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
