//! 変換結果の出力を担当する。
//!
//! `OutputWriter` はファイル書き出し・`mod.rs` 生成・合成型配置・rustfmt を統一的に処理する。
//! 責務は以下のサブモジュールに分割されている:
//!
//! - [`placement`][]: 合成型の配置先決定（inline / shared_module）と shared_imports 構築
//! - [`mod_rs_emit`][]: ディレクトリごとの `mod.rs` 生成（子モジュール宣言と re-export）
//! - この `mod.rs`: entry point `write_to_directory`（書き出し順序の orchestration のみ）

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::module_graph::ModuleGraph;
use super::types::OutputFile;
use crate::ir::Item;

mod mod_rs_emit;
mod placement;

#[cfg(test)]
mod tests;

use mod_rs_emit::collect_dirs;

/// 変換結果の出力を担当する。
///
/// `ModuleGraph` を参照して `mod.rs` を生成し、合成型の配置先を決定し、
/// ファイルをディレクトリに書き出す。
pub struct OutputWriter<'a> {
    module_graph: &'a ModuleGraph,
}

/// 合成型の配置結果。
#[derive(Debug)]
pub struct SyntheticPlacement {
    /// ファイルにインラインで追加する合成型。値は `(name, generated_code)` の組。
    /// I-371: 名前を保持することで推移インポート計算（inline 経由で参照される shared
    /// 型を逆引きする）が可能になる。
    pub inline: HashMap<PathBuf, Vec<(String, String)>>,
    /// 専用モジュールに配置する合成型: (モジュールパス, 合成型コード)
    pub shared_module: Option<(PathBuf, String)>,
    /// 共有モジュールに配置された型を参照するファイルへのインポート文。
    /// I-371: 各合成型は単一正準配置（shared_module）を持ち、参照側ファイルは
    /// `use crate::shared_types::Type;` でインポートする。直接参照だけでなく、
    /// inline 配置された合成型 → shared 配置型への推移参照も含む。
    pub shared_imports: HashMap<PathBuf, Vec<String>>,
}

impl<'a> OutputWriter<'a> {
    /// 新しい `OutputWriter` を構築する。
    pub fn new(module_graph: &'a ModuleGraph) -> Self {
        Self { module_graph }
    }

    /// 変換結果をディレクトリに書き出す。
    ///
    /// ファイル書き出し → 共有モジュール書き出し → `mod.rs` 生成 → rustfmt の順で処理する。
    /// 各ステップは [`placement`][] / [`mod_rs_emit`][] サブモジュール側の関数に委譲し、
    /// この関数は orchestration のみを担当する。
    pub fn write_to_directory(
        &self,
        output_dir: &Path,
        file_outputs: &[OutputFile<'_>],
        synthetic_items: &[Item],
        run_rustfmt: bool,
    ) -> Result<()> {
        let placement = self.resolve_synthetic_placement(file_outputs, synthetic_items);

        // 1. ファイル書き出し（合成型のインライン挿入を含む）
        let mut all_paths = Vec::new();
        for output in file_outputs {
            let rel_path = &output.rel_path;
            let out_path = output_dir.join(rel_path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut content = String::new();
            // I-371: shared_types からのインポートをファイル先頭に追加
            if let Some(imports) = placement.shared_imports.get(rel_path) {
                for import in imports {
                    content.push_str(import);
                    content.push('\n');
                }
                if !imports.is_empty() {
                    content.push('\n');
                }
            }
            // インライン合成型を追加
            if let Some(inline_items) = placement.inline.get(rel_path) {
                for (_name, item_code) in inline_items {
                    content.push_str(item_code);
                    content.push_str("\n\n");
                }
            }
            content.push_str(output.source);

            std::fs::write(&out_path, &content)?;
            all_paths.push(out_path);
        }

        // 2. 共有合成型モジュールの書き出し
        if let Some((types_rel_path, types_code)) = &placement.shared_module {
            let types_path = output_dir.join(types_rel_path);
            if let Some(parent) = types_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&types_path, types_code)?;
            all_paths.push(types_path);
        }

        // 3. mod.rs 生成
        let dirs = collect_dirs(output_dir, file_outputs);
        for dir in &dirs {
            let mod_content = self.generate_mod_rs(dir);
            // 共有合成型モジュールがある場合、ルート mod.rs に pub mod を追加
            let mod_content = if let Some((ref types_rel_path, _)) = placement.shared_module {
                if dir == output_dir {
                    let stem = types_rel_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("shared_types");
                    let mod_decl = format!("pub mod {stem};");
                    if mod_content.is_empty() {
                        mod_decl
                    } else {
                        format!("{mod_content}\n{mod_decl}")
                    }
                } else {
                    mod_content
                }
            } else {
                mod_content
            };
            if !mod_content.is_empty() {
                let mod_rs_path = dir.join("mod.rs");
                std::fs::write(&mod_rs_path, &mod_content)?;
                all_paths.push(mod_rs_path);
            }
        }

        // 4. rustfmt
        if run_rustfmt {
            crate::run_rustfmt(&all_paths);
        }

        Ok(())
    }
}
