"""
Hono ベンチマークエラーのカテゴリ分類ロジック。

analyze-bench.py と inspect-errors.py の共通モジュール。
"""


def categorize(kind: str) -> str:
    """エラー kind 文字列をカテゴリに分類する。

    順序が重要: より具体的なパターンを先にチェックする。
    """
    if "object literal requires" in kind:
        return "OBJECT_LITERAL_NO_TYPE"
    if "type alias body" in kind:
        return "TYPE_ALIAS_UNSUPPORTED"
    if ("Regex" in kind and "literal" in kind.lower()) or "Regex(Regex" in kind:
        return "REGEX_LITERAL"
    if "arrow" in kind and "default" in kind:
        return "ARROW_DEFAULT_PARAM"
    if "arrow parameter pattern" in kind:
        return "ARROW_PARAM_PATTERN"
    if "member property" in kind:
        return "MEMBER_PROPERTY"
    if "indexed access" in kind:
        return "INDEXED_ACCESS"
    if "intersection" in kind:
        return "INTERSECTION_TYPE"
    if "type in union" in kind:
        return "UNION_TYPE"
    # TsNonNull を Null より先にチェック (I-162: 誤分類防止)
    if "TsNonNull" in kind:
        return "TS_NON_NULL"
    if "Null" in kind:
        return "NULL_LITERAL"
    if "no type annotation" in kind:
        return "NO_TYPE_ANNOTATION"
    if "default parameter value" in kind or "default parameter requires" in kind:
        return "DEFAULT_PARAM_VALUE"
    if "binary operator" in kind:
        return "BINARY_OPERATOR"
    if "ForIn" in kind:
        return "FOR_IN_STMT"
    if "multiple declarators" in kind:
        return "FOR_MULTI_DECL"
    if "type literal member" in kind:
        return "TYPE_LITERAL_MEMBER"
    if "call target" in kind:
        return "CALL_TARGET"
    if "TsBigInt" in kind or "BigInt" in kind:
        return "BIGINT"
    if "TsModuleDecl" in kind:
        return "TS_MODULE_DECL"
    if "ExportAll" in kind and "unsupported" in kind.lower() and "Class(" not in kind:
        return "EXPORT_ALL"
    if "for...of binding" in kind:
        return "FOR_OF_BINDING"
    if "object destructuring" in kind:
        return "OBJ_DESTRUCT_NO_TYPE"
    if "call signature" in kind:
        return "CALL_SIGNATURE_PARAM"
    if "function type parameter" in kind:
        return "FN_TYPE_PARAM"
    if "object literal key" in kind or "object literal property" in kind:
        return "OBJECT_LITERAL_KEY"
    if "interface member" in kind:
        return "INTERFACE_MEMBER"
    if "TaggedTpl" in kind:
        return "TAGGED_TEMPLATE"
    if "compound assignment" in kind:
        return "COMPOUND_ASSIGN"
    if "TsUndefinedKeyword" in kind:
        return "UNDEFINED_KEYWORD"
    if "TsTypePredicate" in kind:
        return "TYPE_PREDICATE"
    if "TsSatisfies" in kind or "satisfies" in kind:
        return "SATISFIES_EXPR"
    if "TsTypeQuery" in kind:
        return "TYPEOF_TYPE"
    if "TsTypeOperator" in kind:
        return "TYPE_OPERATOR"
    if "assignment target" in kind:
        return "ASSIGN_TARGET"
    if "qualified type name" in kind:
        return "QUALIFIED_TYPE"
    if "SeqExpr" in kind:
        return "SEQ_EXPR"
    if "Empty" in kind:
        return "EMPTY_STMT"
    return "OTHER"
