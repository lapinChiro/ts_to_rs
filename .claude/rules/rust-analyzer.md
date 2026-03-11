# rust-analyzer MCP サーバーの利用

- 作業開始時に `rust_analyzer_set_workspace` でワークスペースパスを設定すること
- rust-analyzer の diagnostics を「一時的なキャッシュの問題」として安易に無視しない
- `cargo build` が通っていても rust-analyzer のエラーが出ている場合は、まずワークスペース設定を確認する
- 新しいファイル追加後やモジュール構成変更後は `rust_analyzer_diagnostics` で確認する
- 問題を発見したら自発的に原因を調査し、修正すること（ユーザーに指摘される前に）
- サブエージェントがファイルを変更した後は `rust_analyzer_set_workspace` で再読み込みし、diagnostics を確認する
