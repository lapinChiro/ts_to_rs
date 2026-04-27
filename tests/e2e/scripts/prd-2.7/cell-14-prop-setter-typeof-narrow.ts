// PRD 2.7 Matrix Cell #14: Prop::Setter × typeof narrow inside body
//
// Pre-PRD 2.7: TypeResolver で Prop::Setter body 経路不在 (silent drop)
// Post-PRD 2.7 (T9): param_pat visit + body visit_block_stmt 経由 walk
// Transformer: 完全 Tier 1 化は I-202、本 PRD では UnsupportedSyntaxError honest error
//
// E2E note: 本 fixture は Tier 2 honest error 出力 (cargo run fail)、spec-traceable evidence

const config = {
    _name: "" as string,
    _logCount: 0 as number,
    set name(v: string | number) {
        if (typeof v === "string") {
            this._name = v;
        } else {
            this._name = String(v);
        }
        this._logCount += 1;
    }
};

function main(): void {
    config.name = "alice";
    console.log(config._name);      // "alice"
    config.name = 42;
    console.log(config._name);      // "42"
    console.log(config._logCount);  // 2
}
