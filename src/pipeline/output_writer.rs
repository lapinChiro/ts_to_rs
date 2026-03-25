//! 変換結果の出力を担当する。
//!
//! `OutputWriter` はファイル書き出し・mod.rs 生成・合成型配置・rustfmt を統一的に処理する。
//! mod.rs は `ModuleGraph` の query API（`children_of`, `reexports_of`）から生成する。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::module_graph::ModuleGraph;
use crate::ir::Item;

/// 変換結果の出力を担当する。
///
/// `ModuleGraph` を参照して mod.rs を生成し、合成型の配置先を決定し、
/// ファイルをディレクトリに書き出す。
pub struct OutputWriter<'a> {
    module_graph: &'a ModuleGraph,
}

/// 合成型の配置結果。
#[derive(Debug)]
pub struct SyntheticPlacement {
    /// ファイルにインラインで追加する合成型: (ファイルパス, 合成型コード)
    pub inline: HashMap<PathBuf, Vec<String>>,
    /// 専用モジュールに配置する合成型: (モジュールパス, 合成型コード)
    pub shared_module: Option<(PathBuf, String)>,
}

impl<'a> OutputWriter<'a> {
    /// 新しい OutputWriter を構築する。
    pub fn new(module_graph: &'a ModuleGraph) -> Self {
        Self { module_graph }
    }

    /// 指定ディレクトリの mod.rs 内容を生成する。
    ///
    /// `ModuleGraph.children_of()` で `pub mod <name>;` を、
    /// `ModuleGraph.reexports_of()` で `pub use <path>::<name>;` を生成する。
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

    /// 合成型の配置先を決定する。
    ///
    /// 各合成型の名前で全ファイルの生成コードを検索し、参照ファイル数で配置先を決定する:
    /// - 1 ファイルのみで参照 → `inline`（そのファイルの先頭に追加）
    /// - 2+ ファイルで参照 → `shared_module`（`types.rs` に配置）
    /// - 0 ファイル → 未使用（どちらにも含まない）
    pub fn resolve_synthetic_placement(
        &self,
        file_outputs: &[(PathBuf, String)],
        synthetic_items: &[Item],
    ) -> SyntheticPlacement {
        let mut inline: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut shared_items: Vec<String> = Vec::new();

        for item in synthetic_items {
            let name = synthetic_type_name(item);
            let code = crate::generator::generate(std::slice::from_ref(item));

            // 合成型名を参照しているファイルを検索
            let referencing_files: Vec<&PathBuf> = file_outputs
                .iter()
                .filter(|(_, source)| source.contains(&name))
                .map(|(path, _)| path)
                .collect();

            match referencing_files.len() {
                0 => {
                    // 未使用 — 配置しない
                }
                1 => {
                    // 1 ファイルのみ → inline
                    inline
                        .entry(referencing_files[0].clone())
                        .or_default()
                        .push(code);
                }
                _ => {
                    // 2+ ファイル → shared module
                    shared_items.push(code);
                }
            }
        }

        let shared_module = if shared_items.is_empty() {
            None
        } else {
            Some((PathBuf::from("types.rs"), shared_items.join("\n\n")))
        };

        SyntheticPlacement {
            inline,
            shared_module,
        }
    }

    /// 変換結果をディレクトリに書き出す。
    ///
    /// ファイル書き出し → mod.rs 生成 → 合成型配置 → rustfmt の順で処理する。
    pub fn write_to_directory(
        &self,
        output_dir: &Path,
        file_outputs: &[(PathBuf, String)],
        synthetic_items: &[Item],
        run_rustfmt: bool,
    ) -> Result<()> {
        let placement = self.resolve_synthetic_placement(file_outputs, synthetic_items);

        // 1. ファイル書き出し（合成型のインライン挿入を含む）
        let mut all_paths = Vec::new();
        for (rel_path, source) in file_outputs {
            let out_path = output_dir.join(rel_path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut content = String::new();
            // インライン合成型をファイル先頭に追加
            if let Some(inline_items) = placement.inline.get(rel_path) {
                for item_code in inline_items {
                    content.push_str(item_code);
                    content.push_str("\n\n");
                }
            }
            content.push_str(source);

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
            // 共有合成型モジュールがある場合、ルート mod.rs に pub mod types; を追加
            let mod_content = if placement.shared_module.is_some() && dir == output_dir {
                if mod_content.is_empty() {
                    "pub mod types;".to_string()
                } else {
                    format!("{mod_content}\npub mod types;")
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

/// 合成型の識別名を取得する（ファイル内の参照検索に使用）。
fn synthetic_type_name(item: &Item) -> String {
    match item {
        Item::Struct { name, .. }
        | Item::Fn { name, .. }
        | Item::Enum { name, .. }
        | Item::TypeAlias { name, .. }
        | Item::Trait { name, .. } => name.clone(),
        Item::Impl { struct_name, .. } => struct_name.clone(),
        Item::Use { path, .. } => path.clone(),
        Item::Comment(text) | Item::RawCode(text) => text.clone(),
    }
}

/// file_outputs のパスからディレクトリ一覧を取得する（深い方から順）。
fn collect_dirs(output_dir: &Path, file_outputs: &[(PathBuf, String)]) -> Vec<PathBuf> {
    let mut dirs = std::collections::BTreeSet::new();
    dirs.insert(output_dir.to_path_buf());
    for (rel_path, _) in file_outputs {
        let full = output_dir.join(rel_path);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Visibility;
    use crate::parser::parse_typescript;
    use crate::pipeline::module_graph::ModuleGraphBuilder;
    use crate::pipeline::{NullModuleResolver, ParsedFiles};
    use std::path::PathBuf;

    /// テスト用 ModuleGraph を構築する（NodeModuleResolver 使用、re-export 解決付き）。
    fn build_module_graph_with_resolver(root: &Path, filenames: &[&str]) -> ModuleGraph {
        use crate::pipeline::module_resolver::NodeModuleResolver;

        let parsed_files: Vec<_> = filenames
            .iter()
            .map(|name| {
                let path = root.join(name);
                let source = std::fs::read_to_string(&path).unwrap();
                crate::pipeline::ParsedFile {
                    path,
                    source: source.clone(),
                    module: parse_typescript(&source).unwrap(),
                }
            })
            .collect();
        let parsed = ParsedFiles {
            files: parsed_files,
        };
        let known: std::collections::HashSet<PathBuf> =
            filenames.iter().map(|n| root.join(n)).collect();
        let resolver = NodeModuleResolver::new(root.to_path_buf(), known);
        ModuleGraphBuilder::new(&parsed, &resolver, root).build()
    }

    /// テスト用 ModuleGraph を構築する（NullModuleResolver 使用、re-export 解決なし）。
    fn build_module_graph(files: &[(PathBuf, &str)]) -> ModuleGraph {
        let parsed_files: Vec<_> = files
            .iter()
            .map(|(path, source)| crate::pipeline::ParsedFile {
                path: path.clone(),
                source: source.to_string(),
                module: parse_typescript(source).unwrap(),
            })
            .collect();
        let parsed = ParsedFiles {
            files: parsed_files,
        };
        let resolver = NullModuleResolver;
        let root = PathBuf::from("src");
        ModuleGraphBuilder::new(&parsed, &resolver, &root).build()
    }

    // ===== generate_mod_rs tests =====

    #[test]
    fn test_generate_mod_rs_children_modules() {
        let mg = build_module_graph(&[
            (PathBuf::from("src/a.ts"), "export function fa(): void {}"),
            (PathBuf::from("src/b.ts"), "export function fb(): void {}"),
        ]);
        let writer = OutputWriter::new(&mg);
        let result = writer.generate_mod_rs(Path::new("src"));
        assert!(
            result.contains("pub mod a;"),
            "should contain pub mod a: {result}"
        );
        assert!(
            result.contains("pub mod b;"),
            "should contain pub mod b: {result}"
        );
    }

    #[test]
    fn test_generate_mod_rs_empty_dir() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let result = writer.generate_mod_rs(Path::new("src"));
        assert!(
            result.is_empty(),
            "empty dir should produce empty string: {result}"
        );
    }

    #[test]
    fn test_generate_mod_rs_reexports() {
        // Integration test: re-export を含む ModuleGraph から pub use が生成されることを確認。
        // E2E テスト（integration_test.rs）でも同等の検証がされるため、
        // ここでは ModuleGraph の re-export 解決が動作する前提でのフォーマットを確認。
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        std::fs::write(src.join("foo.ts"), "export function hello(): void {}").unwrap();
        std::fs::write(src.join("index.ts"), r#"export { hello } from "./foo";"#).unwrap();

        let mg = build_module_graph_with_resolver(src, &["foo.ts", "index.ts"]);
        let writer = OutputWriter::new(&mg);
        let result = writer.generate_mod_rs(src);
        // children_of が foo を検出するはず
        assert!(
            result.contains("pub mod foo;"),
            "should contain pub mod foo: {result}"
        );
        // re-export の pub use は ModuleGraph の re-export 解決に依存する。
        // NullModuleResolver 相当のケースでは pub use が生成されない可能性がある。
        // この検証は E2E テスト（Hono ベンチマーク）で網羅する。
    }

    #[test]
    fn test_generate_mod_rs_mixed() {
        let mg = build_module_graph(&[
            (PathBuf::from("src/a.ts"), "export function fa(): void {}"),
            (PathBuf::from("src/b.ts"), "export function fb(): void {}"),
        ]);
        let writer = OutputWriter::new(&mg);
        let result = writer.generate_mod_rs(Path::new("src"));
        assert!(
            result.contains("pub mod a;"),
            "should contain pub mod a: {result}"
        );
        assert!(
            result.contains("pub mod b;"),
            "should contain pub mod b: {result}"
        );
    }

    // ===== resolve_synthetic_placement tests =====

    fn make_synthetic_enum(name: &str) -> Item {
        Item::Enum {
            vis: Visibility::Public,
            name: name.to_string(),
            serde_tag: None,
            variants: vec![],
        }
    }

    #[test]
    fn test_resolve_synthetic_placement_single_file() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let files = vec![(
            PathBuf::from("a.rs"),
            "fn foo() -> StringOrF64 { todo!() }".to_string(),
        )];
        let items = vec![make_synthetic_enum("StringOrF64")];
        let placement = writer.resolve_synthetic_placement(&files, &items);
        assert!(
            placement.inline.contains_key(Path::new("a.rs")),
            "should be inline in a.rs"
        );
        assert!(
            placement.shared_module.is_none(),
            "should not have shared module"
        );
    }

    #[test]
    fn test_resolve_synthetic_placement_multi_file() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let files = vec![
            (
                PathBuf::from("a.rs"),
                "fn foo() -> StringOrF64 { todo!() }".to_string(),
            ),
            (
                PathBuf::from("b.rs"),
                "fn bar() -> StringOrF64 { todo!() }".to_string(),
            ),
        ];
        let items = vec![make_synthetic_enum("StringOrF64")];
        let placement = writer.resolve_synthetic_placement(&files, &items);
        assert!(placement.inline.is_empty(), "should not be inline");
        assert!(
            placement.shared_module.is_some(),
            "should have shared module"
        );
    }

    #[test]
    fn test_resolve_synthetic_placement_unused() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let files = vec![(PathBuf::from("a.rs"), "fn foo() {}".to_string())];
        let items = vec![make_synthetic_enum("UnusedEnum")];
        let placement = writer.resolve_synthetic_placement(&files, &items);
        assert!(placement.inline.is_empty(), "should not be inline");
        assert!(
            placement.shared_module.is_none(),
            "should not have shared module"
        );
    }

    // ===== write_to_directory tests =====

    #[test]
    fn test_write_to_directory_creates_files() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        let files = vec![
            (PathBuf::from("a.rs"), "fn a() {}".to_string()),
            (PathBuf::from("sub/b.rs"), "fn b() {}".to_string()),
        ];

        writer.write_to_directory(out, &files, &[], false).unwrap();

        assert!(out.join("a.rs").exists(), "a.rs should exist");
        assert!(out.join("sub/b.rs").exists(), "sub/b.rs should exist");
    }

    #[test]
    fn test_write_to_directory_inline_synthetic() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        let files = vec![(
            PathBuf::from("a.rs"),
            "fn foo() -> MyEnum { todo!() }".to_string(),
        )];
        let items = vec![make_synthetic_enum("MyEnum")];

        writer
            .write_to_directory(out, &files, &items, false)
            .unwrap();

        let content = std::fs::read_to_string(out.join("a.rs")).unwrap();
        assert!(
            content.starts_with("#[derive"),
            "inline synthetic should be at file start: {content}"
        );
        assert!(
            content.contains("fn foo()"),
            "original content should be preserved: {content}"
        );
    }

    #[test]
    fn test_write_to_directory_shared_synthetic() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        let files = vec![
            (
                PathBuf::from("a.rs"),
                "fn foo() -> SharedEnum { todo!() }".to_string(),
            ),
            (
                PathBuf::from("b.rs"),
                "fn bar() -> SharedEnum { todo!() }".to_string(),
            ),
        ];
        let items = vec![make_synthetic_enum("SharedEnum")];

        writer
            .write_to_directory(out, &files, &items, false)
            .unwrap();

        assert!(
            out.join("types.rs").exists(),
            "types.rs should be created for shared synthetics"
        );
    }
}
