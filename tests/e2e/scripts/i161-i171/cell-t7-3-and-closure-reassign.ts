// T7-3: `&&=` interaction with closure reassign (I-144 T6-2 closure-reassign suppression).
// When closure reassigns outer x, narrow should be suppressed. `x &&= 3`
// inside suppressed narrow must still work via new Layer-2 emission path.

function f(): number {
    let x: number | null = 5;
    const reset = () => { x = null; };
    if (x !== null) {
        x &&= 3;  // narrow-suppressed path (closure reset exists)
        reset();
        return x ?? -1;
    }
    return -1;
}

function main(): void {
    console.log(f()); // -1 (reset fires, x becomes null, ?? -1)
}
