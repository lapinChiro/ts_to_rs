// PRD 2.7 Matrix Cell #10: Prop::KeyValue × regression lock-in
//
// 既存 Prop::KeyValue handle の regression lock-in test。
// PRD 2.7 で TypeResolver expressions.rs:331+ ast::Expr::Object arm の `_` arm 削除 +
// 全 Prop variant explicit enumerate に伴い、既存 KeyValue handle が破壊されていない
// ことを E2E で verify。
//
// Pre/post-PRD 2.7 で stdout 完全一致が expected (= TypeResolver の internal 構造 change
// は user-observable runtime behavior に影響しない)。

interface Config {
    name: string;
    version: number;
    debug: boolean;
}

function buildConfig(): Config {
    return {
        name: "ts_to_rs",       // Prop::KeyValue (PropName::Ident)
        version: 1,             // Prop::KeyValue
        debug: true,            // Prop::KeyValue
    };
}

function main(): void {
    const cfg = buildConfig();
    console.log(cfg.name);     // "ts_to_rs"
    console.log(cfg.version);  // 1
    console.log(cfg.debug);    // true
}
