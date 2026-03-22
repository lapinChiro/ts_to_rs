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
/// `f.tctx()` と `f.reg()` が使える。
pub struct TctxFixture {
    mg: ModuleGraph,
    reg: TypeRegistry,
    res: FileTypeResolution,
}

impl TctxFixture {
    /// 空のレジストリ・空の FileTypeResolution で構築する。
    pub fn new() -> Self {
        Self {
            mg: ModuleGraph::empty(),
            reg: TypeRegistry::new(),
            res: FileTypeResolution::empty(),
        }
    }

    /// カスタムレジストリで構築する（型定義を登録済みのテスト用）。
    pub fn with_reg(reg: TypeRegistry) -> Self {
        Self {
            mg: ModuleGraph::empty(),
            reg,
            res: FileTypeResolution::empty(),
        }
    }

    /// カスタム FileTypeResolution で構築する（lookup テスト用）。
    pub fn with_resolution(res: FileTypeResolution) -> Self {
        Self {
            mg: ModuleGraph::empty(),
            reg: TypeRegistry::new(),
            res,
        }
    }

    /// TransformContext を生成する（借用のため呼び出しごとに生成）。
    pub fn tctx(&self) -> TransformContext<'_> {
        TransformContext::new(&self.mg, &self.reg, &self.res, Path::new("test.ts"))
    }

    /// TypeRegistry への参照を返す。
    pub fn reg(&self) -> &TypeRegistry {
        &self.reg
    }
}
