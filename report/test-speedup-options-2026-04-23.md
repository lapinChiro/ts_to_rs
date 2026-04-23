# Test Speedup Options (2026-04-23)

## Summary

現状のボトルネックは `tests/compile_test.rs` と `tests/e2e_test.rs` の 2 つだけで、どちらも「Rust のテスト関数が遅い」のではなく「テスト関数の中で外部プロセスを大量に直列起動している」ことが原因です。

したがって、優先順位は次の通りです。

1. `cargo` 起動回数を減らす
2. 共有作業ディレクトリをやめて安全に並列化する
3. TypeScript oracle を毎回再実行しない
4. 補助的に `nextest` や runner 改善を入れる

`nextest` 単独では本丸は改善しません。まず `compile_test` / `e2e_test` の実行モデルを変える必要があります。

## Detailed Findings

### 1. 現状の遅さの根本原因

#### `compile_test`

`tests/compile_test.rs` は `COMPILE_LOCK` で全体を直列化し、各 fixture ごとに共有の `tests/compile-check` に `src/lib.rs` を書いて `cargo check` を起動しています。

- lock: `tests/compile_test.rs:18-21`
- per-fixture `cargo check`: `tests/compile_test.rs:69-97`
- fixture loops: `tests/compile_test.rs:179-206`, `261-289`, `400-403`

現状の warm baseline では:

- `83` single-file fixtures (`transpile_collecting`)
- `85` single-file fixtures (`transpile_with_builtins`)
- `1` multi-file fixture directory
- 合計 `169` 回の `cargo check`

#### `e2e_test`

`tests/e2e_test.rs` は `E2E_LOCK` で直列化し、各 test case ごとに:

1. TS -> Rust 変換
2. `tests/e2e/rust-runner/src/main.rs` を上書き
3. `cargo run --quiet`
4. `tsx` を起動して TS oracle 実行

を毎回やっています。

- lock: `tests/e2e_test.rs:24-30`
- mtime workaround: `tests/e2e_test.rs:32-55`
- single-file flow: `tests/e2e_test.rs:84-167`
- multi-file flow: `tests/e2e_test.rs:274-361`

通常実行では `132` E2E ケースが active です。

### 2. 改善案一覧

#### A. `compile_test` を shard 化して 1 shard = 1 `cargo check` にする

**要点**

single-file fixture を 1 件ずつ `cargo check` するのをやめ、複数 fixture を 1 crate に束ねて module 化し、shard 単位で `cargo check` します。

**イメージ**

- `tests/compile-check-shards/no_builtins/shard_0/src/*.rs`
- `tests/compile-check-shards/no_builtins/shard_1/src/*.rs`
- `tests/compile-check-shards/with_builtins/shard_0/src/*.rs`
- 各 fixture は `mod fixture_x;`
- failure 時は module/file 名で fixture を特定

**なぜ効くか**

今は `169` 回 `cargo check` を起動しています。これを例えば `8` shard にできれば、Cargo front-end 起動、fingerprint 更新、crate graph 解決、`lib.rs` 再書き込みのオーバーヘッドを大きく削れます。

**期待効果**

- 高い
- `compile_test` の支配コストを最も直接に削る

**難所**

- failure message を fixture 単位で見やすく保つ必要がある
- builtins 有無の 2 系統は分ける必要がある

**評価**

- 影響: very high
- 実装難度: medium
- 優先度: highest

#### B. `compile_test` を isolated workdir で並列化する

**要点**

共有の `tests/compile-check` をやめ、worker ごとに独立した temporary/project dir を使って parallel 実行します。

**イメージ**

- worker 0 -> `/tmp/ts_to_rs_compile_check_0`
- worker 1 -> `/tmp/ts_to_rs_compile_check_1`
- 共通の `Cargo.toml` はコピー
- `CARGO_TARGET_DIR` は共有してもよいし worker ごとでもよい

**なぜ効くか**

今の `COMPILE_LOCK` は shared mutable state のために必要です。workdir を分離すれば lock を外せます。

**期待効果**

- shard 化と組み合わせると大きい
- 単独でも CPU を使えるようになる

**難所**

- Cargo の artifact lock / target dir contention を設計する必要がある
- worker 数は `nproc` に対して調整が必要

**評価**

- 影響: high
- 実装難度: medium
- 優先度: high

#### C. `e2e_test` の Rust runner を worker pool 化して並列実行する

**要点**

共有の `tests/e2e/rust-runner` をやめ、複数の isolated runner dir を持たせて E2E を並列実行可能にします。

**イメージ**

- `tests/e2e/rust-runner-0`
- `tests/e2e/rust-runner-1`
- `tests/e2e/rust-runner-2`
- `tests/e2e/rust-runner-3`

test 名から worker index を決めるか、実行時に pool から配布します。

**なぜ効くか**

今は `E2E_LOCK` と `write_with_advancing_mtime` が必要な時点で、shared mutable project がボトルネックです。runner を分離すれば:

- `E2E_LOCK` を外せる
- `LAST_MTIME` workaround を外せる可能性が高い
- `cargo test --test e2e_test` の並列実行を使える

**関連**

- TODO `I-173` の正面解決候補

**期待効果**

- 高い
- `132` ケースの wall-clock をかなり削れる余地がある

**難所**

- temp TS file (`*_exec.ts`) も worker ごとに分離した方が安全
- multi-file E2E も同じ isolation モデルに載せる必要がある

**評価**

- 影響: very high
- 実装難度: medium-high
- 優先度: high

#### D. `e2e_test` を shard build + dispatcher 実行モデルへ変える

**要点**

各 E2E ケースごとに `cargo run` するのをやめ、複数ケースを 1 回の Rust build にまとめます。ビルド後は dispatcher binary に case 名を渡して実行します。

**イメージ**

- shard ごとに generated crate を作る
- 各 case は `mod case_x;`
- 各 module は `pub fn run() -> Output`
- main binary は `argv[1]` で対象 case を dispatch

**なぜ効くか**

今の E2E は active `132` ケースに対して `cargo run` を `132` 回やっています。Rust build オーバーヘッドがケース単位で繰り返されています。これを shard 数まで圧縮できます。

**期待効果**

- 非常に高い
- E2E の最大改善候補

**難所**

- generated Rust source に `main` が前提の fixture をどう収めるか設計が必要
- stdin / env / stderr 比較 / async main を共通 ABI に載せる必要がある
- 単独失敗時のデバッグ体験を維持する必要がある

**評価**

- 影響: very high
- 実装難度: high
- 優先度: high, but after smaller isolation fixes

#### E. TS oracle を遅延評価 / キャッシュする

**要点**

通常の `e2e_test` では `tsx` を毎回起動せず、fixture ごとの oracle (`stdout` / `stderr`) をファイルとして保持し、TS 実行は必要時だけにします。

**可能な設計**

1. fixture と一緒に `*.expected.stdout`, `*.expected.stderr` を保存
2. `UPDATE_E2E_ORACLES=1` の時だけ `tsx` を実行して再生成
3. あるいは hash cache を持ち、fixture / `package-lock.json` / Node major version が変わった時だけ再評価

**なぜ効くか**

今の E2E は `132` 回 `tsx` を起動しています。TS 側は source-of-truth ですが、fixture が変わっていないのに毎回 oracle を再計算する必要はありません。

**注意**

これは「テストケース削減」ではなく「oracle の再計算を遅延させる」案です。比較件数自体は維持できます。

**関連**

- `scripts/record-cell-oracle.sh` が既に per-cell oracle 記録の方向性を持っている
- `I-180` の async-main harness 問題とも相性がよい。oracle 生成経路を 1 箇所に固定できる

**期待効果**

- medium-high
- `tsx` process 起動コストをほぼ除去できる

**難所**

- oracle 更新フローの規約整備が必要
- TS 実行環境が変わったときの invalidation rule が必要

**評価**

- 影響: high
- 実装難度: medium
- 優先度: high

#### F. TS oracle を常駐 worker 化する

**要点**

`tsx` を case ごとに起動せず、Node worker を 1 プロセス常駐させて fixture path / stdin / env を標準入出力でやり取りします。

**なぜ効くか**

process spawn と CLI 初期化を 132 回繰り返す必要がなくなります。

**注意**

これは oracle cache ほど大きな構造変化ではない一方、毎回 TS 実行の原則を保てます。

**難所**

- worker protocol 設計が必要
- `tsx` CLI を直接使わず、同等の TS 実行経路を自前で持つ必要がある
- 失敗時の再現性を保つ工夫が必要

**評価**

- 影響: medium
- 実装難度: medium-high
- 優先度: medium

#### G. `nextest` 導入

**要点**

`cargo-nextest` を導入し、unit / 普通の integration test を高速に回します。

**なぜ限定的か**

現状の支配コストは:

- `compile_test`: shared project + repeated `cargo check`
- `e2e_test`: shared runner + repeated `cargo run` / `tsx`

なので、libtest を `nextest` に置き換えるだけでは本丸はほぼ残ります。

**効く領域**

- `cargo test --lib`
- `integration_test`
- 小さな integration test 群

**効かない領域**

- `COMPILE_LOCK`, `E2E_LOCK` の中
- case 内部の repeated process execution

**評価**

- 影響: low-medium
- 実装難度: low
- 優先度: medium-low

#### H. `sccache` / build cache を追加する

**要点**

CI / ローカルで `RUSTC_WRAPPER=sccache` を使い、generated crate の再コンパイル結果をキャッシュします。

**なぜ補助策か**

依存 crate は既に Cargo がかなり再利用しています。問題の多くは「Cargo を何度も起動して generated source を差し替える」ことなので、sccache 単独では構造問題を消せません。

**評価**

- 影響: low-medium
- 実装難度: low
- 優先度: low

### 3. 推奨ロードマップ

#### Phase 1: low-risk / high-return

1. `compile_test` を shard 化する
2. `compile_test` workdir を worker ごとに分けて並列化する
3. `e2e_test` runner を worker pool 化して `E2E_LOCK` を外す

この段階で、現状の shared mutable project 起因の直列ボトルネックをかなり崩せます。

#### Phase 2: structural E2E reduction

4. TS oracle の cache / lazy refresh を導入する
5. 余力があれば E2E を shard build + dispatcher モデルへ移行する

E2E は `cargo run` 回数削減が最も効くため、本気で 1 分未満を狙うならここまで入る価値があります。

#### Phase 3: supplementary

6. `cargo-nextest`
7. `sccache`

これは Phase 1/2 の後に足すのが妥当です。

### 4. 私なら最初にやる案

最初の 1 本としては、`compile_test` shard 化を選びます。

理由:

- 支配コスト (`106s`) が明確
- `compile_test` は TS oracle や async semantics を含まず、E2E より実装リスクが低い
- failure mode が「compile error」で単純
- 成功すれば E2E 側に同じ batching / isolation の発想を展開しやすい

2 本目は `e2e_test` の runner pool 化です。これは `I-173` と速度改善を同時に扱えます。

## References

- `report/test-execution-baseline-2026-04-23.md`
- `tests/compile_test.rs:18-21`
- `tests/compile_test.rs:69-97`
- `tests/compile_test.rs:179-206`
- `tests/compile_test.rs:220-289`
- `tests/compile_test.rs:304-403`
- `tests/e2e_test.rs:24-30`
- `tests/e2e_test.rs:32-55`
- `tests/e2e_test.rs:84-167`
- `tests/e2e_test.rs:274-361`
- `tests/e2e/package.json`
- `tests/e2e/rust-runner/Cargo.toml`
- `TODO:680` (`I-173`)
- `TODO:692` (`I-180`)
- `TODO:752`
