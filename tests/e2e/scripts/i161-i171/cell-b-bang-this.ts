// I-171 Layer 1 Cell B.1.35 (This operand): `!this.x` in class context.
// Ideal: falsy_predicate_for_expr on self's field type.

class Box {
    value: number;
    constructor(v: number) {
        this.value = v;
    }

    isFalsy(): boolean {
        return !this.value;
    }
}

function main(): void {
    console.log(new Box(0).isFalsy());   // true
    console.log(new Box(5).isFalsy());   // false
    console.log(new Box(NaN).isFalsy()); // true
}
