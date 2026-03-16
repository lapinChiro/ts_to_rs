function main(): void {
    const s: string = "Hello, World!";
    console.log("upper:", s.toUpperCase());
    console.log("lower:", s.toLowerCase());
    console.log("includes:", s.includes("World"));
    console.log("starts:", s.startsWith("Hello"));
    console.log("trim:", "  spaces  ".trim());
    console.log("split:", "a,b,c".split(",").join(" "));

    // 文字列結合 (I-56 fix)
    const name: string = "Rust";
    const greeting: string = "Hello " + name;
    console.log(greeting);
}
