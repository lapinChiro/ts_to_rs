// PRD 2.7 Matrix Cell #13: Prop::Getter × typeof narrow inside body
//
// Pre-PRD 2.7: TypeResolver で Prop::Getter body 経路不在 (silent drop)
// Post-PRD 2.7 (T9): body visit_block_stmt 経由 walk で narrow event push
// Transformer: 完全 Tier 1 化は I-202、本 PRD では UnsupportedSyntaxError honest error
//
// E2E note: 本 fixture は Tier 2 honest error 出力 (cargo run fail)、spec-traceable evidence

const data = {
    _raw: "hello" as string | number,
    get displayValue(): string {
        if (typeof this._raw === "string") {
            return this._raw.toUpperCase();
        }
        return String(this._raw);
    }
};

function main(): void {
    console.log(data.displayValue);  // "HELLO" (string branch)
}
