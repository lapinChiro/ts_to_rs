# rust-analyzer MCP サーバーの利用

## トリガー

1. 作業開始時
2. ファイルの追加・削除やモジュール構成の変更後
3. サブエージェントがファイルを変更した後

## アクション

- **作業開始時**: `rust_analyzer_set_workspace` でワークスペースパスを設定する
- **構成変更後・サブエージェント変更後**: `rust_analyzer_set_workspace` で再読み込みし、`rust_analyzer_diagnostics` で確認する
- **diagnostics にエラーがある場合**: `cargo build` の成否に関わらず、まずワークスペース設定を確認し、それでも解消しなければ原因を調査・修正する

## 禁止事項

- rust-analyzer の diagnostics を「一時的なキャッシュの問題」として無視すること
- `cargo build` が通ることを理由に rust-analyzer のエラーを放置すること
- diagnostics の確認をせずに作業を完了とすること
