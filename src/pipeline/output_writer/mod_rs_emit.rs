//! ディレクトリごとの `mod.rs` 生成ロジック。
//!
//! `ModuleGraph` の query API (`children_of`, `reexports_of`) から、各ディレクトリに
//! 配置すべき `pub mod <name>;` / `pub use <path>::<name>;` を組み立てる。
//!
//! - [`OutputWriter::generate_mod_rs`]: 単一ディレクトリの mod.rs 内容を生成
//! - [`collect_dirs`]: `file_outputs` のパス一覧から対象ディレクトリ一覧を深い方から列挙

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::super::types::OutputFile;
use super::OutputWriter;

impl<'a> OutputWriter<'a> {
    /// 指定ディレクトリの `mod.rs` 内容を生成する。
    ///
    /// `ModuleGraph::children_of()` で `pub mod <name>;` を、
    /// `ModuleGraph::reexports_of()` で `pub use <path>::<name>;` を生成する。
    pub fn generate_mod_rs(&self, dir_path: &Path) -> String {
        let mut lines = Vec::new();

        // 子モジュールの pub mod 宣言
        for child in self.module_graph.children_of(dir_path) {
            lines.push(format!("pub mod {};", child));
        }

        // re-export の pub use 宣言
        // reexports_of は dir の index.ts（ディレクトリの代表ファイル）から取得
        // index.ts, index.tsx のいずれかを試す
        let reexports = {
            let candidates = ["index.ts", "index.tsx"];
            candidates
                .iter()
                .flat_map(|name| {
                    let index_path = dir_path.join(name);
                    self.module_graph.reexports_of(&index_path)
                })
                .collect::<Vec<_>>()
        };
        if !reexports.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new()); // pub mod と pub use の間に空行
            }
            for reexport in &reexports {
                lines.push(format!(
                    "pub use {}::{};",
                    reexport.module_path, reexport.name
                ));
            }
        }

        lines.join("\n")
    }
}

/// `file_outputs` のパスから対象ディレクトリ一覧を取得する（深い方から順）。
///
/// `BTreeSet` で浅い方から順に収集した後、`rev()` で深い方から返す。これは `mod.rs`
/// 書き出し時に、子モジュールが存在する親ディレクトリより先に子を処理したほうが
/// 追加のディレクトリ作成が不要になるため。
pub(super) fn collect_dirs(output_dir: &Path, file_outputs: &[OutputFile<'_>]) -> Vec<PathBuf> {
    let mut dirs = BTreeSet::new();
    dirs.insert(output_dir.to_path_buf());
    for output in file_outputs {
        let full = output_dir.join(&output.rel_path);
        if let Some(parent) = full.parent() {
            let mut p = parent.to_path_buf();
            while p != *output_dir && p.starts_with(output_dir) {
                dirs.insert(p.clone());
                p = match p.parent() {
                    Some(pp) => pp.to_path_buf(),
                    None => break,
                };
            }
        }
    }
    // 深い方から順（BTreeSet は浅い方から順なので reverse）
    dirs.into_iter().rev().collect()
}
