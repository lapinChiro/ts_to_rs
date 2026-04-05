# I-367: type_converter パスの前方参照問題

## 背景

Batch 10 で型パラメータ制約のモノモーフィゼーションを実装した。TypeDef パス（resolve_typedef）では 2-pass collection により前方参照が解決されるため、全ての非 trait 制約が正しくモノモーフィゼーションされる。

しかし Item パス（type_converter: convert_interface_items, convert_type_alias 等）は SWC AST の宣言順に逐次処理するため、後方で定義される型が `is_valid_trait_bound` や `lookup_field_type` 呼び出し時に TypeRegistry/SyntheticTypeRegistry に未登録となる。

## 影響

Hono types.rs（dir compile 唯一の失敗ファイル）で 7 エラーが残存:

| エラー | 件数 | 原因 |
|--------|------|------|
| E0404 | 4 | `is_valid_trait_bound` が未登録型を `true`（trait 仮定）と返す → モノモーフィゼーション非適用 |
| E0405 | 1 | `lookup_field_type` が未解決の型に対し `None` → `Input::out` fallback |
| E0107 | 2 | モノモーフィゼーション非適用の型にトランケートが効かない |

dir compile 156/158 → 157/158 のゲート。

## 根本原因

変換パイプラインに 2 つの並行パスが存在し、型定義の利用可能性が異なる:

1. **TypeDef パス** (collection.rs → resolve_typedef): 2-pass。Pass 1 で全型名をプレースホルダー登録、Pass 2 で型を解決。前方参照は Pass 1 の名前で解決可能。

2. **Item パス** (type_converter → generator): 1-pass。transformer が宣言を逐次処理し、convert_interface_items / convert_type_alias を呼ぶ。この時点で TypeRegistry は完全だが、SyntheticTypeRegistry は処理済み宣言分のみ。合成型（union enum 等）が後方定義の場合、未登録。

具体的な失敗パターン:
```
// TS (types.ts)
interface ArrayBufferView<TArrayBuffer extends ArrayBufferOrSharedArrayBuffer> { ... }
// ↑ extract_type_params 時に ArrayBufferOrSharedArrayBuffer がまだ SyntheticTypeRegistry に未登録
// → is_valid_trait_bound が true → モノモーフィゼーション非適用

type ArrayBufferOrSharedArrayBuffer = ArrayBuffer | SharedArrayBuffer;
// ↑ 後で処理される → SyntheticTypeRegistry に登録
```

## 解決策候補

### 案 1: Item 生成を全型の TypeDef 登録完了後に実行

現在 transformer は宣言を逐次処理し、TypeDef 登録と Item 生成を同時に行う。これを分離し、全 TypeDef 登録（+ 合成型登録）完了後に Item 生成パスを実行する。

**利点**: 最小の変更で前方参照を解決。TypeRegistry + SyntheticTypeRegistry が完全な状態で Item パスが実行される。
**欠点**: 2 パス化により処理時間が増加（ただし SWC AST の再走査が必要）。

### 案 2: TypeDef パスから直接 Item を生成

TypeDef を Item に変換する関数を作成し、type_converter パスを廃止。resolve_typedef の結果から直接 IR Item を生成する。

**利点**: パスの重複を根本的に解消。DRY。
**欠点**: type_converter の全ロジック（interface → trait/struct 判定、type alias の各種パターン認識等）を TypeDef → Item 変換に移植する必要があり、大規模リファクタリング。

### 案 3: type_converter にも 2-pass 処理を導入

type_converter 専用の前方参照解決パスを追加。Pass 1 で型名と基本情報を収集、Pass 2 で Item を生成。

**利点**: type_converter の既存ロジックを活かせる。
**欠点**: 2-pass のインフラが 2 箇所に存在する重複。

### 推奨

**案 1** が最もコスト対効果が高い。transformer の `convert_module_items` で宣言の処理順序を変更するだけで、既存の型変換ロジックに手を入れずに解決できる可能性がある。

## 完了条件

1. Hono types.rs の E0404/E0405/E0107 が全て解消
2. dir compile 156 → 157+
3. 既存テスト全 pass
4. 前方参照を含む fixture テスト追加

## 関連

- Batch 10 レポート: `report/batch10-rc15-type-param-context.md`
- `is_valid_trait_bound`: `src/ts_type_info/resolve/typedef.rs`
- `lookup_field_type`: `src/ts_type_info/resolve/indexed_access.rs:271`
- `convert_interface_items`: `src/pipeline/type_converter/interfaces.rs`
- `convert_type_alias`: `src/pipeline/type_converter/type_aliases.rs`
