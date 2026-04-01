# I-312b: convert_ts_type を TsTypeInfo 経由に統一 — registry 責務逸脱の完全解消

## 進捗

- **Phase 1〜3**: 完了（convert_ts_type の 2 ステップ化達成、全 1,809 テスト pass）
- **Phase 4〜5**: 未着手（Registry の TsTypeInfo 移行 + 検証クリーンアップ）

## 背景

I-312 の Phase A〜C で以下の基盤を構築済み:
- `FieldDef<T>` / `ParamDef<T>`: optional / has_default メタデータ保持
- `TsTypeInfo`: SWC AST 非依存の TS 型表現
- `TypeDef<T = RustType>`: ジェネリック化されたデフォルト型パラメータ
- `convert_to_ts_type_info`: TsType → TsTypeInfo 変換
- `resolve_ts_type` / `resolve_typedef`: TsTypeInfo → RustType 変換（基本型のみ）

**残課題**: registry モジュールは依然として `convert_ts_type` を直接呼び出しており、
Option ラップと PascalCase 命名が registry フェーズに残っている。

`convert_ts_type` は 6 ファイル・3,042 行・30+ 関数で構成される複雑なサブシステム
（ユーティリティ型、intersection 合成、discriminated union 検出、indexed access、
conditional type 等）。`resolve_ts_type` が `convert_ts_type` と完全等価でなければ、
registry の TsTypeInfo 移行で回帰が発生する。

## 設計

### 方針: convert_ts_type を TsTypeInfo 経由の 2 ステップに書き換える

`convert_ts_type` を「SWC AST → TsTypeInfo → RustType」の合成に書き換える。
これにより変換ロジックが `resolve_ts_type` に統一され、registry は
`convert_to_ts_type_info` のみを使用する構造が実現する。

```
// Before (現在)
convert_ts_type(ts_type: &TsType, ...) → RustType  // 3,042 行の直接変換

// After (目標)
convert_ts_type(ts_type: &TsType, ...) → RustType {
    let info = convert_to_ts_type_info(ts_type)?;  // ステップ 1: 構文マッピング
    resolve_ts_type(&info, reg, synthetic)          // ステップ 2: 意味解決
}
```

### TsTypeInfo の拡張

現在の `TsTypeInfo::ObjectLiteral` はプロパティのみ保持している。
intersection 合成やトレイト変換にはメソッドシグネチャ等が必要。

```rust
// 拡張: ObjectLiteral → TypeLiteral
pub struct TsTypeLiteralInfo {
    pub fields: Vec<TsFieldInfo>,
    pub methods: Vec<TsMethodInfo>,
    pub call_signatures: Vec<TsFnSigInfo>,
    pub construct_signatures: Vec<TsFnSigInfo>,
    pub index_signatures: Vec<TsIndexSigInfo>,
}

pub struct TsMethodInfo {
    pub name: String,
    pub params: Vec<TsParamInfo>,
    pub return_type: Option<TsTypeInfo>,
    pub type_params: Vec<String>,
}

pub struct TsFnSigInfo {
    pub params: Vec<TsParamInfo>,
    pub return_type: Option<TsTypeInfo>,
}

pub struct TsParamInfo {
    pub name: String,
    pub ty: TsTypeInfo,
    pub optional: bool,
}

pub struct TsIndexSigInfo {
    pub param_name: String,
    pub param_type: TsTypeInfo,
    pub value_type: TsTypeInfo,
    pub readonly: bool,
}
```

### convert_ts_type サブモジュールの移行

各サブモジュールの関数を TsTypeInfo ベースに書き換える。
ロジックは同一、入力データのアクセスパターンのみ変更。

| サブモジュール | 行数 | 主要関数 | 移行内容 |
|---------------|------|---------|---------|
| `mod.rs` | 395 | `convert_ts_type`, `convert_type_ref`, `resolve_keyof_type` | TsTypeInfo 入力に変更。`convert_ts_type` は 2 ステップ合成に |
| `unions.rs` | 611 | `convert_union_type`, `try_convert_discriminated_union`, `try_convert_string_literal_union`, `try_convert_general_union` | TsTypeInfo::Union 入力。PascalCase 命名はここに集約（registry から移動済み） |
| `intersections.rs` | 784 | `try_convert_intersection_type`, `convert_type_lit_in_annotation`, `convert_intersection_in_annotation`, `convert_fn_type` | TsTypeInfo::Intersection, TypeLiteral 入力。method/call sig の TsMethodInfo 活用 |
| `indexed_access.rs` | 259 | `convert_indexed_access_type` | TsTypeInfo::IndexedAccess 入力 |
| `utilities.rs` | 475 | `convert_utility_partial/required/pick/omit/non_nullable` | TsTypeInfo::TypeRef 入力。TypeRegistry 参照は維持（フィールド解決用） |
| `type_aliases.rs` | 518 | `convert_type_alias`, `convert_conditional_type` | TsTypeInfo 入力。条件型の infer パターン解析 |

### registry 移行

サブモジュールの TsTypeInfo 化完了後:

1. `registry/interfaces.rs`: `convert_ts_type` → `convert_to_ts_type_info` に置換。Option ラップ除去。
2. `registry/functions.rs`: 同上。デフォルトパラメータ Option ラップ除去。
3. `registry/unions.rs`: `string_to_pascal_case` 除去（型変換フェーズに集約済み）。
4. `registry/collection.rs`: クラスフィールド・コンストラクタの変換を TsTypeInfo に。
5. `build_registry_with_synthetic`: TypeDef<TsTypeInfo> 構築 → `resolve_typedef` で TypeDef 変換。

### 意味論的安全性

`convert_ts_type` と `resolve_ts_type` の出力が同一であることを保証する:

1. **回帰テスト**: 既存の全テスト（1764 件）が pass
2. **Hono ベンチマーク**: clean 数・error instances が回帰しない
3. **変換結果比較テスト**: 代表的な TS 型パターンで `convert_ts_type` と
   `convert_to_ts_type_info → resolve_ts_type` の出力が一致することを検証

## 実装フェーズ

### Phase 1: TsTypeInfo 拡張 ✅

- `TsTypeLiteralInfo`, `TsMethodInfo`, `TsFnSigInfo`, `TsParamInfo`, `TsIndexSigInfo` 定義
- `TsTypeInfo::ObjectLiteral` → `TsTypeInfo::TypeLiteral(TsTypeLiteralInfo)` に拡張
- `convert_to_ts_type_info` を拡張して method/call/construct/index sig を収集
- `TsTypeInfo::Mapped` に `has_readonly`, `has_optional`, `name_type` 追加
- `TsTypeInfo::Infer`, `TsTypeInfo::Symbol` variant 追加

### Phase 2: resolve_ts_type の完全化 ✅

`resolve/` サブモジュール構造に分割し、convert_ts_type のロジックを移植:

- `resolve/union.rs`: nullable → Option、string literal → String、multi-type → synthetic enum（AST 順名前生成）
- `resolve/intersection.rs`: フィールドマージ + 重複検出 + メソッド impl 生成 + union 分配（discriminated 対応）
- `resolve/indexed_access.rs`: string key → フィールド型/associated type、number index → const 要素型、keyof typeof → 値型 union、TypeLiteral ベース、ネスト対応、union key
- `resolve/utility.rs`: Partial/Required/Pick/Omit/NonNullable（nested utility 対応: resolve_inner_fields_with_conversion）
- `resolve/conditional.rs`: infer パターン + 型述語 + フォールバック

### Phase 3: convert_ts_type を 2 ステップ合成に書き換え ✅

```rust
pub fn convert_ts_type(ts_type: &TsType, synthetic: &mut SyntheticTypeRegistry, reg: &TypeRegistry) -> Result<RustType> {
    let info = convert_to_ts_type_info(ts_type)?;
    resolve_ts_type(&info, reg, synthetic)
}
```

- 旧ディスパッチコード（convert_ts_type_legacy, resolve_keyof_type）削除
- 旧サブモジュール（indexed_access, intersections, unions）の未使用関数に `#[allow(dead_code)]`
- Promise<T> は Named("Promise") のまま返す（unwrap は transformer の責務）
- identity mapped type の修飾子・name_type チェック（symbol filter noop 判定）
- 全 1,809 テスト pass（ユニット 1,627 + コンパイル 3 + スナップショット 3 + E2E 84 + integration 89）

### Phase 4: Registry の TsTypeInfo 移行（未着手）

- registry 関数が `convert_to_ts_type_info` のみ呼び出す
- Option ラップ除去（`resolve_field_def` に委譲）
- PascalCase 除去（`resolve_ts_type` の union 処理に委譲）
- `build_registry_with_synthetic` で `TypeDef<TsTypeInfo>` → `TypeDef` 変換
- 旧サブモジュールの `#[allow(dead_code)]` 関数を完全削除

### Phase 5: 検証 + クリーンアップ（未着手）

- 全テスト pass
- Hono ベンチマーク回帰チェック
- /quality-check

## 完了基準

1. ✅ **`convert_ts_type` が `convert_to_ts_type_info` + `resolve_ts_type` の合成である**
2. ⬜ **registry モジュール内に `convert_ts_type` / `convert_type_for_position` の呼び出しが 0 件** — Phase 4
3. ⬜ **registry モジュール内に `RustType::Option(Box::new(...))` のラップコードが 0 件** — Phase 4
4. ⬜ **registry モジュール内に `string_to_pascal_case` の呼び出しが 0 件** — Phase 4
5. ✅ **`resolve_ts_type` が `convert_ts_type` の全パターンをカバー**
6. ✅ **全既存テストが pass**（1,809 件）
7. ⬜ **Hono ベンチマークが回帰していない（clean 数 ≥ 110, error instances ≤ 58）** — Phase 5
8. ✅ **cargo clippy, cargo fmt --check が 0 warnings**
