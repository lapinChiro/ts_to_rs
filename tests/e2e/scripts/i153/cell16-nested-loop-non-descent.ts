// I-153 matrix cell #16: bare break inside nested loop (non-descent).
// The walker MUST NOT rewrite inner loop's bare break, which correctly targets
// the inner loop in both TS and Rust.

function f(x: number): number {
    let count = 0;
    for (let i = 0; i < 2; i++) {
        switch (x) {
            case 1:
                for (let j = 0; j < 5; j++) {
                    if (j >= 2) break;   // inner loop break, NOT switch break
                    count = count + 1;
                }
                count = count + 100;
                break;
            default:
                count = count + 1000;
                break;
        }
        count = count + 10;
    }
    return count;
}

function main(): void {
    // f(1): 2 iter × (inner j: 0,1 → count += 2, inner break at j=2 → count+=100, outer +=10) = 2 × 112 = 224
    console.log(f(1));
    // f(2): 2 × (1000 + 10) = 2020
    console.log(f(2));
}
