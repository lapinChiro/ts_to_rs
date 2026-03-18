# I-24: 外部パッケージの型定義解決 — 調査レポート

**基準コミット**: 12c69c0（未コミットの変更あり）
**調査日**: 2026-03-18

## 1. 要約

I-24（外部パッケージの型定義解決）は、TODO 上で多くの項目（I-49/50/51, I-112b）のブロッカーとして参照されているが、**調査の結果、問題の大部分は I-24 なしで解決可能**であることが判明した。

### 核心の発見

1. **ディレクトリモードが既に存在する**: `build_shared_registry()` で複数ファイルの型をマージする仕組みが実装済み。Hono をディレクトリとして変換すれば、`SmartRouter`, `serialize` 等のプロジェクト内の型は自動解決される
2. **I-112b の 12 ファイル中、真に I-24 が必要なのは Web API ビルトイン（Response, ReadableStream, crypto）のみ**。残りはディレクトリモードで解決可能
3. **Hono の変換テストが単一ファイルモードで行われていた**ため、プロジェクト内の型参照が全て「未登録」に見えていた

## 2. 現在の実装状態

### 2.1 import の処理

`transform_import` (mod.rs:347) で外部パッケージ import を**明示的にスキップ**:

```rust
if !src.starts_with("./") && !src.starts_with("../") {
    return None;  // 外部パッケージは無視
}
```

相対パス import のみ `use crate::module::Name;` に変換される。

### 2.2 TypeRegistry の構築

| モード | レジストリの範囲 | API |
|--------|----------------|-----|
| 単一ファイル | 対象ファイルのみ | `transpile(source)` |
| ディレクトリ | 全 `.ts` ファイル | `transpile_with_registry(source, shared_registry)` |

ディレクトリモードでは `build_shared_registry()` (lib.rs:32) が全ファイルの型定義をマージする。

### 2.3 CLI の対応

```bash
ts_to_rs file.ts    # 単一ファイル（型は file.ts 内のみ）
ts_to_rs src/       # ディレクトリ（src/ 内の全 .ts ファイルの型を共有）
```

## 3. I-112b の 12 ファイルの再分類

| 未登録型 | 使用例 | 定義元 | ディレクトリモードで解決? |
|---------|--------|-------|----------------------|
| SmartRouter | `new SmartRouter(...)` | Hono 内部 (`router/smart-router/`) | **はい** |
| serialize | `serialize(name, value, opts)` | Hono 内部 (`utils/cookie.ts`) | **はい** |
| hc | `hc(url, opts)` | Hono 内部 (`client/`) | **はい** |
| createPool | `createPool(opts)` | Hono 内部 (`utils/concurrent.ts`) | **はい** |
| Response | `new Response(msg, init)` | Web API ビルトイン | **いいえ** |
| ReadableStream | `new ReadableStream(...)` | Web API ビルトイン | **いいえ** |
| crypto.subtle | `crypto.subtle.digest(...)` | Web API ビルトイン | **いいえ** |

**結論**: 12 ファイル中、真に外部型解決が必要なのは **Web API ビルトイン型を使う約 4〜5 ファイルのみ**。残りはディレクトリモードで変換すれば解決する。

## 4. Web API ビルトイン型の対応方法

真に必要な外部型は `Response`, `ReadableStream`, `crypto.subtle` の 3 つ。

### 方法 A: ユーザー提供の型スタブファイル

ユーザーが `types/web-api.ts` のような型定義ファイルを用意:

```typescript
interface ResponseInit {
    status?: number;
    statusText?: string;
    headers?: Record<string, string>;
}
```

CLI で `--with-types types/` オプションを追加し、スタブファイルの型も共有レジストリに含める。

### 方法 B: ビルトイン型ライブラリのバンドル

よく使われる Web API 型（Response, Request, Headers 等）をツール内にハードコードまたはバンドル。

### 方法 C: lib.dom.d.ts の解析

TypeScript の `lib.dom.d.ts` を解析して TypeRegistry に登録。ただし巨大（2万行以上）で、全て登録するのは非現実的。

### 推奨: 方法 A が最も KISS

- 既存の `build_shared_registry()` を拡張するだけ
- ユーザーが必要な型だけ定義すれば良い
- CLI に `--with-types` オプション追加は軽微

## 5. Hono 変換テストへの示唆

**現在の Hono 変換テストは単一ファイルモードで実行している**（`./target/release/ts_to_rs file.ts`）。これにより:

- プロジェクト内の型参照が全て「未登録」に見える
- 実際にはディレクトリモードで解決可能な問題が、I-24 のブロッカーとして誤分類されている

**hono-cycle スキルでディレクトリモードのテストを追加すべき**:

```bash
# 単一ファイルモードの変換率（現在の計測方法）
./target/release/ts_to_rs file.ts

# ディレクトリモードの変換率（追加すべき計測方法）
./target/release/ts_to_rs /tmp/hono-src/src/
```

ディレクトリモードで変換率がどれだけ改善するかを計測することで、I-24 の真の影響範囲が明確になる。

## 6. TODO への影響

### I-24 の再定義

現在の I-24 は「外部パッケージの型定義解決」と広く定義されているが、実態は:

1. **ディレクトリモードの活用**（既に実装済み）で大部分が解決
2. **Web API ビルトイン型**のみが真の未解決課題
3. **npm パッケージの .d.ts 解析**は現時点では不要（Hono は自己完結的）

### I-112b の再評価

I-112b は「I-24 が前提」として保留されているが:

- ディレクトリモードで 7〜8 件は解決可能
- 残り 4〜5 件のみが真に外部型解決を必要とする
- 保留を解除し、段階的に対応可能

### I-49/50/51 の再評価

E2E テスト基盤（stdin, ファイル I/O, HTTP）も I-24 ブロッカーとされているが、ディレクトリモードの活用で一部解除可能か要検討。
