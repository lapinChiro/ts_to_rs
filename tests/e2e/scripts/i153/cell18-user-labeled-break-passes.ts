// I-153 matrix cell #18: user-labeled `break L;` inside switch case body.
// The walker must NOT rewrite labeled breaks — user's intent is to target `L`.

function f(x: number): number {
    let count = 0;
    outer: for (let i = 0; i < 10; i++) {
        switch (x) {
            case 1:
                if (i >= 2) break outer;   // user intent: break OUTER loop
                count = count + 100;
                break;
            default:
                count = count + 1;
                break;
        }
        count = count + 10;
    }
    return count;
}

function main(): void {
    // f(1): outer loop runs i=0,1 (count += 110 each), at i=2 break outer → 220
    console.log(f(1));
    // f(2): all 10 iter × (1 + 10) = 110
    console.log(f(2));
}
