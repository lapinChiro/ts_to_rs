function main(): void {
    // Static types — typeof resolves at compile time
    const n: number = 42;
    console.log("typeof number:", typeof n);

    const s: string = "hello";
    console.log("typeof string:", typeof s);

    const b: boolean = true;
    console.log("typeof boolean:", typeof b);

    // Option type — typeof resolves at runtime
    const present: number | undefined = 42;
    console.log("typeof present:", typeof present);

    const absent: number | undefined = undefined;
    console.log("typeof absent:", typeof absent);
}
