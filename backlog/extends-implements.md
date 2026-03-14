# PRD: `extends` と `implements` の併用対応

## Background

`class Child extends Parent implements Greeter` パターンが未対応。`extends` 単独（`generate_items_for_class`）と `implements` 単独（`generate_class_with_implements`）は実装済みだが、`transform_class_with_inheritance` の分岐が排他的であるため、組み合わせの dispatch がない。

現状の `src/transformer/mod.rs` `transform_class_with_inheritance`（438-471行目付近）:

1. `is_abstract` チェック
2. `parent_names.contains`（親クラスか）
3. `info.parent`（子クラスか）
4. `info.implements`（インタフェース実装か）

これらが排他分岐のため、extends と implements を両方持つクラスは子クラス分岐に入り、implements 情報が失われる。

## Goal

`extends` と `implements` を併用するクラスが以下を生成する:

- struct（親のフィールドを含む）
- `impl Child`（super 書き換え付き）
- `impl ParentTrait for Child`
- `impl InterfaceTrait for Child`（インタフェースごと）

## Scope

- **IN**: `transform_class_with_inheritance` に extends+implements 複合分岐を追加
- **IN**: インタフェース由来メソッドを `impl Interface for Child` に、残りを `impl Child` に分配
- **OUT**: abstract class + implements の組み合わせ（別の concern）

## Steps

1. **RED**: `class Child extends Parent implements Greeter` のテストを追加（期待: struct + impl + impl ParentTrait + impl Greeter）
2. **GREEN**: `transform_class_with_inheritance` で `info.parent.is_some() && !info.implements.is_empty()` の分岐を追加
3. **E2E**: フィクスチャファイル `tests/fixtures/extends-implements.input.ts` を追加
4. **Quality check**

## Test plan

- extends + 単一 implements: struct + impl + trait impl 2つ（parent + interface）が生成される
- extends + 複数 implements: interface ごとに trait impl が生成される
- リグレッション: extends のみ・implements のみが既存と同一出力

## Completion criteria

- extends+implements クラスが正しい struct + impl ブロック群を生成する
- 全テスト pass、0 errors / 0 warnings
