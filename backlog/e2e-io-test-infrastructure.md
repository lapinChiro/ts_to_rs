# E2E テスト基盤: I/O テスト（stdin / ファイル / HTTP）

対象 TODO: I-49, I-50, I-51

## 背景・動機

現在の E2E テストは「TS ソースコードをファイルから読み込み → 変換 → stdout 比較」という単一のパターンのみ。実際のアプリケーションで頻出する以下の I/O パターンがテストできない:

1. **標準入力**: stdin から入力を受け取り、処理結果を stdout に出力するプログラム
2. **ファイル I/O**: ファイルの読み書きを行うプログラム
3. **HTTP**: HTTP リクエストを送信し、レスポンスを処理するプログラム

これらは変換機能の実用性を検証するために不可欠だが、テスト基盤自体が対応していない。

## ゴール

- stdin を入力として受け取る E2E テストが実行可能になっている
- ファイルの読み書きを行う E2E テストが実行可能になっている
- HTTP リクエスト/レスポンスを行う E2E テストが実行可能になっている
- 各 I/O パターンについて最低 1 つのサンプルテストが PASS している

## スコープ

### 対象

- `tests/e2e_test.rs` への stdin 対応テスト関数追加
- `tests/e2e_test.rs` へのファイル I/O 対応テスト関数追加
- `tests/e2e_test.rs` への HTTP 対応テスト関数追加
- 各 I/O パターンのサンプルスクリプト作成
- rust-runner の `Cargo.toml` への依存追加（必要に応じて）

### 対象外

- TS の `fs` / `http` / `readline` 等のモジュール変換ロジックの実装（変換機能側は別 PRD）
- 既存テストのリファクタリング
- 非同期 I/O の対応（同期 I/O のみ対象）

## 設計

### 技術的アプローチ

#### stdin テスト

- 新関数 `run_e2e_test_with_stdin(name: &str, stdin_input: &str)` を追加
- TS 側: `tsx` にパイプで stdin を渡す（`echo "input" | tsx script.ts`）
- Rust 側: `cargo run` にパイプで stdin を渡す
- stdout を行単位で比較

TS 側の stdin 読み取り:
```typescript
// readline を使わず、process.stdin から同期的に読む
const input = require('fs').readFileSync('/dev/stdin', 'utf8');
```

Rust 側の変換結果:
```rust
use std::io::Read;
let mut input = String::new();
std::io::stdin().read_to_string(&mut input).unwrap();
```

#### ファイル I/O テスト

- 新関数 `run_e2e_test_with_file_io(name: &str)` を追加
- テスト前にテンポラリディレクトリを作成
- TS 側: `tsx` に環境変数 `TEST_DIR` でテンポラリディレクトリを渡す
- Rust 側: 同じ環境変数を渡す
- テスト後にテンポラリディレクトリをクリーンアップ
- stdout を行単位で比較

TS 側のファイル操作:
```typescript
import * as fs from 'fs';
const dir = process.env.TEST_DIR!;
fs.writeFileSync(`${dir}/test.txt`, "hello");
const content = fs.readFileSync(`${dir}/test.txt`, 'utf8');
```

Rust 側の変換結果:
```rust
let dir = std::env::var("TEST_DIR").unwrap();
std::fs::write(format!("{}/test.txt", dir), "hello").unwrap();
let content = std::fs::read_to_string(format!("{}/test.txt", dir)).unwrap();
```

#### HTTP テスト

- 新関数 `run_e2e_test_http(name: &str)` を追加
- テストランナーが簡易 HTTP サーバーを起動し、ポートを環境変数 `TEST_PORT` で渡す
- TS 側: `fetch` でリクエスト
- Rust 側: `reqwest::blocking::get` でリクエスト
- stdout を行単位で比較
- テスト後にサーバーを停止

### 影響範囲

- `tests/e2e_test.rs` (関数追加)
- `tests/e2e/scripts/` (サンプルスクリプト追加)
- `tests/e2e/rust-runner/Cargo.toml` (依存追加: reqwest 等)

## 作業ステップ

- [ ] ステップ 1: `run_e2e_test_with_stdin` 関数を実装
- [ ] ステップ 2: stdin サンプルスクリプト（`stdin_echo.ts`）を作成し、E2E テストで動作確認
- [ ] ステップ 3: `run_e2e_test_with_file_io` 関数を実装（テンポラリディレクトリ管理含む）
- [ ] ステップ 4: ファイル I/O サンプルスクリプト（`file_io.ts`）を作成し、E2E テストで動作確認
- [ ] ステップ 5: テストランナー内の簡易 HTTP サーバーを実装
- [ ] ステップ 6: `run_e2e_test_http` 関数を実装
- [ ] ステップ 7: HTTP サンプルスクリプト（`http_request.ts`）を作成し、E2E テストで動作確認
- [ ] ステップ 8: 既存 E2E テスト全件の退行チェック

## テスト計画

- **stdin**: 入力文字列をそのまま出力するエコー。複数行入力。空入力
- **ファイル I/O**: ファイルの書き込み→読み込み→内容出力。ファイル存在確認
- **HTTP**: GET リクエスト→ステータスコード・レスポンスボディの出力
- **退行テスト**: 既存 E2E テスト全件が PASS すること

## 完了条件

- [ ] stdin / ファイル I/O / HTTP の各テスト関数が実装済み
- [ ] 各 I/O パターンのサンプルテストが PASS
- [ ] 既存 E2E テストに退行がないこと
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
