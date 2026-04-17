//! Shared test fixtures for transformer tests.
//!
//! `TctxFixture` は TransformContext と TypeRegistry の所有者を提供し、
//! テストごとのボイラープレートを排除する。

use std::path::Path;

use crate::pipeline::type_resolution::FileTypeResolution;
use crate::pipeline::ModuleGraph;
use crate::registry::TypeRegistry;
use crate::transformer::context::TransformContext;

/// Test fixture: TransformContext + TypeRegistry の所有者。
///
/// テスト関数で `let f = TctxFixture::new();` と 1 行書くだけで
/// `f.tctx()` が使える。
///
/// `from_source` / `from_source_with_reg` で構築した場合、TypeResolver 経由で
/// expected type が設定された FileTypeResolution と、解析済み Module を保持する。
/// テストでは `f.module()` から式を抽出し、`convert_expr` で変換すると
/// TypeResolver が設定した expected type が自動的に適用される。
pub struct TctxFixture {
    mg: ModuleGraph,
    reg: TypeRegistry,
    res: FileTypeResolution,
    module: Option<swc_ecma_ast::Module>,
}

impl TctxFixture {
    /// 空のレジストリ・空の FileTypeResolution で構築する。
    pub fn new() -> Self {
        Self {
            mg: ModuleGraph::empty(),
            reg: TypeRegistry::new(),
            res: FileTypeResolution::empty(),
            module: None,
        }
    }

    /// カスタムレジストリで構築する（型定義を登録済みのテスト用）。
    pub fn with_reg(reg: TypeRegistry) -> Self {
        Self {
            mg: ModuleGraph::empty(),
            reg,
            res: FileTypeResolution::empty(),
            module: None,
        }
    }

    /// TS ソースコードを解析し、TypeResolver を実行して TransformContext を構築する。
    ///
    /// unit test で TypeResolver 経由の expected type 設定をテストする場合に使用。
    /// ソースから TypeRegistry と FileTypeResolution が自動的に構築される。
    /// `module()` で解析済み Module にアクセスでき、式の抽出に使用する。
    pub fn from_source(source: &str) -> Self {
        let module = crate::parser::parse_typescript(source).unwrap();
        let reg = crate::registry::build_registry(&module);
        Self::build_with_resolver(module, reg, source)
    }

    /// TS ソースコードを解析し、カスタム TypeRegistry と TypeResolver を使用して構築する。
    ///
    /// ソースに含まれる型定義に加え、事前に登録した型定義（struct のフィールド型等）を
    /// TypeResolver に提供したい場合に使用する。ソースから抽出した型定義は `reg` にマージされる。
    pub fn from_source_with_reg(source: &str, mut reg: TypeRegistry) -> Self {
        let module = crate::parser::parse_typescript(source).unwrap();
        let source_reg = crate::registry::build_registry(&module);
        reg.merge(&source_reg);
        Self::build_with_resolver(module, reg, source)
    }

    /// TypeResolver を実行して TctxFixture を構築する内部ヘルパー。
    fn build_with_resolver(module: swc_ecma_ast::Module, reg: TypeRegistry, source: &str) -> Self {
        let mg = ModuleGraph::empty();
        let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
        let parsed = crate::pipeline::ParsedFile {
            path: std::path::PathBuf::from("test.ts"),
            source: source.to_string(),
            module,
        };
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let res = resolver.resolve_file(&parsed);
        let module = parsed.module;
        Self {
            mg,
            reg,
            res,
            module: Some(module),
        }
    }

    /// TransformContext を生成する（借用のため呼び出しごとに生成）。
    pub fn tctx(&self) -> TransformContext<'_> {
        TransformContext::new(&self.mg, &self.reg, &self.res, Path::new("test.ts"))
    }

    /// 解析済み Module への参照を返す。
    ///
    /// `from_source` / `from_source_with_reg` で構築した場合のみ利用可能。
    /// テストで変数宣言の initializer や式を抽出する際に使用する。
    /// この Module から抽出した式は、FileTypeResolution と同じ span を持つため、
    /// `convert_expr` が TypeResolver の設定した expected type を正しく読み取る。
    ///
    /// # Panics
    ///
    /// `new()` / `with_reg()` で構築した場合はパニックする。
    pub fn module(&self) -> &swc_ecma_ast::Module {
        self.module
            .as_ref()
            .expect("module() is only available on fixtures created with from_source()")
    }

    /// 借用した TypeRegistry から TransformContext を生成するためのヘルパー部品。
    ///
    /// `TctxFixture::with_reg()` は所有権を要求するため、借用のみ持つヘルパー関数
    /// （例: `convert_single_stmt(reg: &TypeRegistry)`）ではこのタプルを使う。
    /// 返り値の `ModuleGraph` と `FileTypeResolution` は TransformContext が借用するため、
    /// TransformContext と同じスコープで保持する必要がある。
    pub fn empty_context_parts() -> (ModuleGraph, FileTypeResolution) {
        (ModuleGraph::empty(), FileTypeResolution::empty())
    }

    /// TS ソースを変換し、IR と生成コードを返す。
    ///
    /// context.rs のテストで使用する統合テスト用ヘルパー。
    pub fn transform(&self, source: &str) -> (Vec<crate::ir::Item>, String) {
        let module = crate::parser::parse_typescript(source).unwrap();
        let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
        let items = crate::transformer::transform_module_with_context(
            &module,
            &self.tctx(),
            &mut synthetic,
        )
        .unwrap();
        let output = crate::generator::generate(&items);
        (items, output)
    }

    /// TS ソースを変換し、IR と [`UnsupportedSyntaxError`] のリストを返す。
    ///
    /// `transform` が不支援項目でパニックするのに対し、本メソッドは
    /// `transform_module_collecting_with_context` を呼ぶため、テストは
    /// `UnsupportedSyntaxError` の `kind` / `byte_pos` を観察できる。
    /// I-142 Cell #5 / #9 / #14 (blocked-by-I-050 / I-144) のような
    /// surface-as-unsupported cell の lock-in に使用する。
    pub fn transform_collecting(
        &self,
        source: &str,
    ) -> (
        Vec<crate::ir::Item>,
        Vec<crate::transformer::UnsupportedSyntaxError>,
    ) {
        let module = crate::parser::parse_typescript(source).unwrap();
        let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
        crate::transformer::transform_module_collecting_with_context(
            &module,
            &self.tctx(),
            &mut synthetic,
        )
        .unwrap()
    }
}
