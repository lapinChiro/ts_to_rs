// I-154: user's label `try_block` (valid TS identifier) must still target user's
// own loop, not our internal `__ts_try_block` labeled block. After I-154 rename,
// internal uses `__ts_try_block`, so user's `try_block` has its own independent
// loop label in Rust.

function f(): number {
    let count = 0;
    try_block: for (let i = 0; i < 5; i++) {
        if (i >= 2) break try_block;
        count = count + 1;
    }
    return count;
}

function main(): void {
    console.log(f());  // expect 2
}
