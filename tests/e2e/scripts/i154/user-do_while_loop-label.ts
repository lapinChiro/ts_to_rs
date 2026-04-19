// I-154 matrix cell #23: user's label `do_while_loop` (valid TS identifier)
// collides with the pre-I-154 internal fallback label used by do-while emission
// when user didn't provide a label. After `__ts_` rename, internal uses
// `__ts_do_while_loop`, so user's `do_while_loop` works independently.

function f(): number {
    let count = 0;
    do_while_loop: for (let i = 0; i < 5; i++) {
        if (i >= 4) break do_while_loop;
        count = count + 1;
    }
    return count;
}

function main(): void {
    console.log(f());  // expect 4
}
