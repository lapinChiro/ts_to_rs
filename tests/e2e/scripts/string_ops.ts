function main(): void {
    const s: string = "Hello, World!";
    console.log("upper:", s.toUpperCase());
    console.log("lower:", s.toLowerCase());
    console.log("includes:", s.includes("World"));
    console.log("starts:", s.startsWith("Hello"));
    console.log("trim:", "  spaces  ".trim());
    console.log("split:", "a,b,c".split(",").join(" "));
}
