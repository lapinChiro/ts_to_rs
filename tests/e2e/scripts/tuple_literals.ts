function main(): void {
    // Array of tuples — tuple literals compile correctly
    const entries: [string, number][] = [["a", 1], ["b", 2], ["c", 3]];
    console.log("count:", entries.length);

    // Single tuple assignment compiles
    const pair: [number, number] = [10, 20];
    console.log("pair type ok");

    // Nested: array of tuples iteration
    for (const entry of entries) {
        console.log("has entry");
    }
}
