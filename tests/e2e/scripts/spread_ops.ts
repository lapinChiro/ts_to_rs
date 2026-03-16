function main(): void {
    // array spread
    const a: number[] = [1, 2, 3];
    const b: number[] = [4, 5, 6];
    const merged: number[] = [...a, ...b];
    console.log("merged length:", merged.length);

    // array spread with elements
    const withExtra: number[] = [0, ...a, 99];
    console.log("with extra length:", withExtra.length);
}
