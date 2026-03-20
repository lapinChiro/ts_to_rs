//! Type environment for tracking local variable types during transformation.

use std::collections::HashMap;

use crate::ir::RustType;
use crate::registry::TypeRegistry;

/// Wraps a trait type for parameter position: `Greeter` → `&dyn Greeter`.
///
/// Non-trait types are returned unchanged.
pub(crate) fn wrap_trait_for_param(ty: RustType, reg: &TypeRegistry) -> RustType {
    if let RustType::Named {
        ref name,
        ref type_args,
    } = ty
    {
        if type_args.is_empty() && reg.is_trait_type(name) {
            return RustType::Named {
                name: format!("&dyn {name}"),
                type_args: vec![],
            };
        }
    }
    ty
}

/// Wraps a trait type for value position (variable, return): `Greeter` → `Box<dyn Greeter>`.
///
/// Non-trait types are returned unchanged.
pub(crate) fn wrap_trait_for_value(ty: RustType, reg: &TypeRegistry) -> RustType {
    if let RustType::Named {
        ref name,
        ref type_args,
    } = ty
    {
        if type_args.is_empty() && reg.is_trait_type(name) {
            return RustType::Named {
                name: format!("Box<dyn {name}>"),
                type_args: vec![],
            };
        }
    }
    ty
}

/// ローカル変数の型情報を保持する型環境。
///
/// スコープチェーンにより、ブロックスコープでの変数シャドウイングを正しく追跡する。
/// 変数宣言時にエントリを追加し、後続の式変換で参照する。
#[derive(Debug, Clone)]
pub struct TypeEnv {
    scopes: Vec<HashMap<String, RustType>>,
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }
}

impl TypeEnv {
    /// 新しい空の型環境を作成する。ルートスコープが 1 つ含まれる。
    pub fn new() -> Self {
        Self::default()
    }

    /// 新しい子スコープを開始する。
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// 現在のスコープを終了し、その中の変数を破棄する。
    /// ルートスコープは pop しない。
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// 変数の型を現在のスコープに登録する。同スコープ内の同名変数は上書きされる。
    pub fn insert(&mut self, name: String, ty: RustType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    /// 既存の変数の型を更新する。スコープチェーンを内側から探索し、
    /// 最初に見つかったスコープで更新する。どのスコープにも存在しない場合は
    /// 現在のスコープに挿入する。
    pub fn update(&mut self, name: String, ty: RustType) {
        for scope in self.scopes.iter_mut().rev() {
            if let std::collections::hash_map::Entry::Occupied(mut e) = scope.entry(name.clone()) {
                e.insert(ty);
                return;
            }
        }
        // どのスコープにも存在しない → 現在のスコープに挿入
        self.insert(name, ty);
    }

    /// 変数名から型を取得する。最内スコープから順に探索する。
    pub fn get(&self, name: &str) -> Option<&RustType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
}
