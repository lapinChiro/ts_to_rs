---
paths:
  - "src/transformer/**"
  - "src/generator/**"
  - "src/ir.rs"
---

# パイプライン整合性

## 適用条件

変換パイプライン（parser → transformer → generator）に関わるコードを追加・変更するとき。

## 制約

- **IR は構造化データで表現する**: IR の型（`Item::*`, `RustType` 等）に表示用に整形された文字列を格納しない。文字列化は generator の責務
- **パイプラインの依存方向を守る**: transformer は IR を生成する。generator は IR を消費する。transformer が `crate::generator` を import してはならない（テストコードを除く）
- **IR に新しいフィールドを追加するとき、全ての Item バリアントに一貫して適用する**: 例えば `type_params` を `Item::Trait` に追加するなら、`Item::Struct`, `Item::Fn`, `Item::TypeAlias` にも同じ構造化型で追加する
- **新しい解決メカニズム（例: `instantiate`）を実装したら、使用箇所への統合テストを書く**: 単体テストだけでは統合漏れを検出できない

## 禁止事項

- transformer 内で `crate::generator::types::generate_type` 等の generator 関数を呼び出すこと
- `Vec<String>` に `"T: Bound"` のような整形済み文字列を格納して IR として扱うこと（代わりに構造体を使う）
- 新しいメソッド（例: `instantiate`）を実装して単体テストだけ書き、変換パイプラインへの統合テストを書かないこと
