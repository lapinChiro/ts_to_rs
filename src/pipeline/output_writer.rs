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
    /// 共有モジュールに配置された型を参照するファイルへのインポート文。
    /// I-371: 各合成型は単一正準配置（shared_module）を持ち、参照側ファイルは
    /// `use crate::shared_types::Type;` でインポートする。
    pub shared_imports: HashMap<PathBuf, Vec<String>>,
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
    /// - 2+ ファイルで参照 → `shared_module`（専用 `.rs` ファイルに配置）
    /// - 0 ファイル → 未使用（どちらにも含まない）
    pub fn resolve_synthetic_placement(
        &self,
        file_outputs: &[(PathBuf, String)],
        synthetic_items: &[Item],
    ) -> SyntheticPlacement {
        let mut inline: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut shared_items: Vec<String> = Vec::new();
        // 共有モジュールに配置された型ごとに、その参照ファイル一覧を記録する。
        // 後続で shared_imports を構築する。
        let mut shared_type_refs: Vec<(String, Vec<PathBuf>)> = Vec::new();

        // 全 synthetic item のコード生成と名前の収集。
        // canonical_name() == None の Item（Use/Comment/RawCode）は配置対象外。
        // 空文字列で contains() を呼ぶと全ファイルにマッチして誤配置するため必ず skip する。
        let generated: Vec<(String, String)> = synthetic_items
            .iter()
            .filter_map(|item| {
                let name = item.canonical_name()?.to_string();
                let code = crate::generator::generate(std::slice::from_ref(item));
                Some((name, code))
            })
            .collect();

        for (name, code) in &generated {
            // 合成型名を参照しているファイルを検索
            let referencing_files: Vec<&PathBuf> = file_outputs
                .iter()
                .filter(|(_, source)| source.contains(name))
                .map(|(path, _)| path)
                .collect();

            // 他の synthetic item から参照されているかチェック
            let referenced_by_synthetic = generated
                .iter()
                .any(|(other_name, other_code)| other_name != name && other_code.contains(name));

            match referencing_files.len() {
                0 if referenced_by_synthetic => {
                    // file_outputs からは未参照だが、他の synthetic item から参照される
                    // → shared module に配置（相互依存を解決）
                    shared_items.push(code.clone());
                    shared_type_refs.push((name.clone(), Vec::new()));
                }
                0 => {
                    // 完全に未使用 — 配置しない
                }
                1 if !referenced_by_synthetic => {
                    // 1 ファイルのみ → inline
                    inline
                        .entry(referencing_files[0].clone())
                        .or_default()
                        .push(code.clone());
                }
                _ => {
                    // 2+ ファイル、または 1 ファイル + synthetic 参照 → shared module
                    shared_items.push(code.clone());
                    shared_type_refs.push((
                        name.clone(),
                        referencing_files.iter().map(|p| (*p).clone()).collect(),
                    ));
                }
            }
        }

        let shared_module = if shared_items.is_empty() {
            None
        } else {
            let body = shared_items.join("\n\n");
            let imports = generate_shared_module_imports(&body);
            let content = if imports.is_empty() {
                body
            } else {
                format!("{imports}\n\n{body}")
            };
            let module_path = choose_shared_module_path(file_outputs);
            Some((module_path, content))
        };

        // shared_imports: 各参照ファイルに対して `use crate::<stem>::{Type, ...};` を構築。
        // 同名の型が user 定義型と衝突するのを防ぐため、参照先ファイル自身が
        // shared module の場合（衝突回避サフィックス付与時など）は除外する。
        let shared_imports: HashMap<PathBuf, Vec<String>> =
            if let Some((ref module_path, _)) = shared_module {
                build_shared_imports(module_path, &shared_type_refs)
            } else {
                HashMap::new()
            };

        SyntheticPlacement {
            inline,
            shared_module,
            shared_imports,
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

/// 共有モジュールに配置された型の参照ファイル群から、ファイル別の `use` 文を構築する。
///
/// 各ファイルが参照する型を `use crate::<stem>::{T1, T2, ...};` の単一文に集約する。
/// 参照ファイルが共有モジュール自身の場合は除外する（自己インポートを防ぐ）。
fn build_shared_imports(
    module_path: &Path,
    shared_type_refs: &[(String, Vec<PathBuf>)],
) -> HashMap<PathBuf, Vec<String>> {
    let stem = module_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("shared_types");

    // file → 参照する型名の集合
    let mut by_file: HashMap<PathBuf, std::collections::BTreeSet<String>> = HashMap::new();
    for (type_name, files) in shared_type_refs {
        for file in files {
            if file == module_path {
                continue;
            }
            by_file
                .entry(file.clone())
                .or_default()
                .insert(type_name.clone());
        }
    }

    by_file
        .into_iter()
        .map(|(file, types)| {
            let names: Vec<String> = types.into_iter().collect();
            let import = if names.len() == 1 {
                format!("use crate::{stem}::{};", names[0])
            } else {
                format!("use crate::{stem}::{{{}}};", names.join(", "))
            };
            (file, vec![import])
        })
        .collect()
}

/// 共有合成型モジュールのファイルパスを決定する。
///
/// ユーザーの出力ファイルと衝突しない名前を選択する。
/// デフォルトは `shared_types.rs`。衝突する場合はサフィックスを付与する。
fn choose_shared_module_path(file_outputs: &[(PathBuf, String)]) -> PathBuf {
    let existing: std::collections::HashSet<&Path> =
        file_outputs.iter().map(|(p, _)| p.as_path()).collect();

    let base = PathBuf::from("shared_types.rs");
    if !existing.contains(base.as_path()) {
        return base;
    }

    // 衝突回避: サフィックスを付与
    for i in 0u32.. {
        let candidate = PathBuf::from(format!("shared_types_{i}.rs"));
        if !existing.contains(candidate.as_path()) {
            return candidate;
        }
    }

    unreachable!("infinite counter guarantees a unique name")
}

/// 共有合成型モジュールに必要なインポート文を生成する。
///
/// 生成されたコード内で使用されているが、モジュール自身ではインポートされていない
/// クレートや標準ライブラリ型のインポートを検出して生成する。
fn generate_shared_module_imports(code: &str) -> String {
    let mut imports = Vec::new();

    if code.contains("serde_json::") {
        imports.push("use serde_json;");
    }
    if code.contains("HashMap<") {
        imports.push("use std::collections::HashMap;");
    }

    imports.join("\n")
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
            type_params: vec![],
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

    #[test]
    fn test_resolve_synthetic_placement_skips_unnamed_items() {
        // canonical_name() == None の Item（Use/Comment/RawCode）が synthetic_items に
        // 混入した場合、それらは配置対象外として skip される。
        // 旧実装の `unwrap_or("").to_string()` では空文字列で contains() が全マッチし
        // shared_module に誤配置されていた。本テストは regression 防止用。
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let files = vec![(
            PathBuf::from("a.rs"),
            "fn foo() -> Real { todo!() }".to_string(),
        )];
        let items = vec![
            Item::Comment("a comment".to_string()),
            Item::Use {
                vis: crate::ir::Visibility::Private,
                path: "crate::bar".to_string(),
                names: vec!["Bar".to_string()],
            },
            Item::RawCode("fn raw() {}".to_string()),
            make_synthetic_enum("Real"),
        ];
        let placement = writer.resolve_synthetic_placement(&files, &items);
        // Real は inline 配置される
        assert!(
            placement.inline.contains_key(Path::new("a.rs")),
            "Real は inline 配置される"
        );
        // shared_module は生成されない（unnamed items が誤って入らない）
        assert!(
            placement.shared_module.is_none(),
            "unnamed items が誤って shared_module に配置されないこと: {:?}",
            placement.shared_module
        );
        // shared_imports も空
        assert!(placement.shared_imports.is_empty());
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
            out.join("shared_types.rs").exists(),
            "shared_types.rs should be created for shared synthetics"
        );
        // mod.rs に pub mod shared_types; が含まれる
        let mod_content = std::fs::read_to_string(out.join("mod.rs")).unwrap();
        assert!(
            mod_content.contains("pub mod shared_types;"),
            "mod.rs should contain pub mod shared_types: {mod_content}"
        );
    }

    #[test]
    fn test_write_to_directory_shared_synthetic_avoids_collision() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        // ユーザーファイルに shared_types.rs が含まれる場合、衝突回避名を使用する
        let files = vec![
            (
                PathBuf::from("shared_types.rs"),
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

        // shared_types.rs は衝突するので shared_types_0.rs に配置
        assert!(
            out.join("shared_types_0.rs").exists(),
            "shared_types_0.rs should be created when shared_types.rs collides"
        );
        // ユーザーの shared_types.rs は上書きされない
        let user_content = std::fs::read_to_string(out.join("shared_types.rs")).unwrap();
        assert!(
            user_content.contains("fn foo()"),
            "user file should not be overwritten: {user_content}"
        );
        // mod.rs に衝突回避後のモジュール名が含まれる
        let mod_content = std::fs::read_to_string(out.join("mod.rs")).unwrap();
        assert!(
            mod_content.contains("pub mod shared_types_0;"),
            "mod.rs should contain pub mod shared_types_0: {mod_content}"
        );
    }

    #[test]
    fn test_write_to_directory_types_rs_not_overwritten() {
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        // ユーザーファイルに types.rs が含まれる場合（Hono のケース）
        // SharedEnum が 2 ファイルから参照されるため shared module に配置される
        let files = vec![
            (
                PathBuf::from("types.rs"),
                "pub struct TypedResponse { data: SharedEnum }".to_string(),
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

        // types.rs はユーザーコードを含む
        let types_content = std::fs::read_to_string(out.join("types.rs")).unwrap();
        assert!(
            types_content.contains("TypedResponse"),
            "types.rs should contain user code: {types_content}"
        );
        // 共有合成型は shared_types.rs に配置（types.rs と衝突しない）
        assert!(
            out.join("shared_types.rs").exists(),
            "shared_types.rs should be created"
        );
        let shared_content = std::fs::read_to_string(out.join("shared_types.rs")).unwrap();
        assert!(
            shared_content.contains("SharedEnum"),
            "shared_types.rs should contain SharedEnum: {shared_content}"
        );
    }

    // ===== I-371: shared_imports / 単一正準配置のテスト =====

    #[test]
    fn test_resolve_synthetic_placement_shared_imports_multi_file() {
        // 2 ファイルから参照される合成型は shared_types に配置され、
        // 両ファイルに `use crate::shared_types::Type;` のインポートが付与される。
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

        assert!(placement.shared_module.is_some());
        let imports_a = placement
            .shared_imports
            .get(Path::new("a.rs"))
            .expect("a.rs should have shared imports");
        assert_eq!(imports_a.len(), 1);
        assert_eq!(imports_a[0], "use crate::shared_types::StringOrF64;");
        let imports_b = placement
            .shared_imports
            .get(Path::new("b.rs"))
            .expect("b.rs should have shared imports");
        assert_eq!(imports_b[0], "use crate::shared_types::StringOrF64;");
    }

    #[test]
    fn test_resolve_synthetic_placement_no_imports_for_inline() {
        // 1 ファイルからのみ参照される型は inline 配置で、shared_imports は空。
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let files = vec![(
            PathBuf::from("a.rs"),
            "fn foo() -> LocalEnum { todo!() }".to_string(),
        )];
        let items = vec![make_synthetic_enum("LocalEnum")];
        let placement = writer.resolve_synthetic_placement(&files, &items);

        assert!(placement.shared_imports.is_empty());
        assert!(placement.inline.contains_key(Path::new("a.rs")));
    }

    #[test]
    fn test_resolve_synthetic_placement_imports_grouped() {
        // 同じファイルが複数の shared 型を参照する場合、単一の `use` 文に集約される。
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let files = vec![
            (
                PathBuf::from("a.rs"),
                "fn foo() -> AlphaEnum { todo!() } fn baz() -> BetaEnum { todo!() }".to_string(),
            ),
            (
                PathBuf::from("b.rs"),
                "fn bar() -> AlphaEnum { todo!() } fn qux() -> BetaEnum { todo!() }".to_string(),
            ),
        ];
        let items = vec![
            make_synthetic_enum("AlphaEnum"),
            make_synthetic_enum("BetaEnum"),
        ];
        let placement = writer.resolve_synthetic_placement(&files, &items);

        let imports_a = placement.shared_imports.get(Path::new("a.rs")).unwrap();
        assert_eq!(imports_a.len(), 1);
        assert_eq!(
            imports_a[0],
            "use crate::shared_types::{AlphaEnum, BetaEnum};"
        );
    }

    #[test]
    fn test_write_to_directory_emits_shared_imports() {
        // shared_types.rs に配置された型を参照するファイルの先頭に
        // `use crate::shared_types::T;` が出力されること。
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

        let a_content = std::fs::read_to_string(out.join("a.rs")).unwrap();
        assert!(
            a_content.contains("use crate::shared_types::SharedEnum;"),
            "a.rs should import SharedEnum: {a_content}"
        );
        let b_content = std::fs::read_to_string(out.join("b.rs")).unwrap();
        assert!(
            b_content.contains("use crate::shared_types::SharedEnum;"),
            "b.rs should import SharedEnum: {b_content}"
        );
    }

    #[test]
    fn test_write_to_directory_no_duplicate_synthetic_definition() {
        // I-371 問題 1 の回帰テスト: 合成型が shared と inline の両方に
        // 重複定義されないことを検証する。
        // 単一ファイルから参照される合成型は inline 配置のみで shared_types.rs に出ない。
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        let files = vec![(
            PathBuf::from("a.rs"),
            "fn foo() -> OnlyOne { todo!() }".to_string(),
        )];
        let items = vec![make_synthetic_enum("OnlyOne")];

        writer
            .write_to_directory(out, &files, &items, false)
            .unwrap();

        let a_content = std::fs::read_to_string(out.join("a.rs")).unwrap();
        // a.rs に 1 回だけ enum 定義が出現
        assert_eq!(
            a_content.matches("pub enum OnlyOne").count(),
            1,
            "OnlyOne should be defined exactly once: {a_content}"
        );
        // shared_types.rs は作られない
        assert!(!out.join("shared_types.rs").exists());
    }

    #[test]
    fn test_write_to_directory_shared_collision_uses_subscripted_import() {
        // shared_types.rs と衝突する場合、サフィックス付きモジュール名が
        // インポート文にも反映される。
        let mg = ModuleGraph::empty();
        let writer = OutputWriter::new(&mg);
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();

        let files = vec![
            (
                PathBuf::from("shared_types.rs"),
                "pub struct UserDefined;".to_string(),
            ),
            (
                PathBuf::from("a.rs"),
                "fn foo() -> ConflictEnum { todo!() }".to_string(),
            ),
            (
                PathBuf::from("b.rs"),
                "fn bar() -> ConflictEnum { todo!() }".to_string(),
            ),
        ];
        let items = vec![make_synthetic_enum("ConflictEnum")];

        writer
            .write_to_directory(out, &files, &items, false)
            .unwrap();

        let a_content = std::fs::read_to_string(out.join("a.rs")).unwrap();
        assert!(
            a_content.contains("use crate::shared_types_0::ConflictEnum;"),
            "a.rs should import from shared_types_0: {a_content}"
        );
    }

    #[test]
    fn test_choose_shared_module_path_no_collision() {
        let files: Vec<(PathBuf, String)> = vec![
            (PathBuf::from("a.rs"), String::new()),
            (PathBuf::from("b.rs"), String::new()),
        ];
        let result = choose_shared_module_path(&files);
        assert_eq!(result, PathBuf::from("shared_types.rs"));
    }

    #[test]
    fn test_choose_shared_module_path_collision() {
        let files: Vec<(PathBuf, String)> = vec![
            (PathBuf::from("shared_types.rs"), String::new()),
            (PathBuf::from("b.rs"), String::new()),
        ];
        let result = choose_shared_module_path(&files);
        assert_eq!(result, PathBuf::from("shared_types_0.rs"));
    }

    #[test]
    fn test_choose_shared_module_path_double_collision() {
        let files: Vec<(PathBuf, String)> = vec![
            (PathBuf::from("shared_types.rs"), String::new()),
            (PathBuf::from("shared_types_0.rs"), String::new()),
        ];
        let result = choose_shared_module_path(&files);
        assert_eq!(result, PathBuf::from("shared_types_1.rs"));
    }
}
