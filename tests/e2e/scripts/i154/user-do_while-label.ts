// I-154: user's label `do_while` (valid TS identifier) should work independently
// of the internal `__ts_do_while` label.

function f(): number {
    let count = 0;
    do_while: for (let i = 0; i < 5; i++) {
        if (i >= 3) break do_while;
        count = count + 10;
    }
    return count;
}

function main(): void {
    console.log(f());  // expect 30
}
