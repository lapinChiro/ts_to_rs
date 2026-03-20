function main(): void {
    const s: string = "Hello, World!";
    console.log("upper:", s.toUpperCase());
    console.log("lower:", s.toLowerCase());
    console.log("includes:", s.includes("World"));
    console.log("starts:", s.startsWith("Hello"));
    console.log("trim:", "  spaces  ".trim());
    console.log("split:", "a,b,c".split(",").join(" "));

    // 文字列結合 
    const name: string = "Rust";
    const greeting: string = "Hello " + name;
    console.log(greeting);

    // String replace: first occurrence only
    const repeated: string = "aaa bbb aaa";
    const replaced: string = repeated.replace("aaa", "ccc");
    console.log("replace:", replaced);

    // split returns Vec<String>
    const parts: string[] = "x-y-z".split("-");
    console.log("split parts:", parts.join(","));

    // substring
    const sub1: string = "abcdef".substring(1, 4);
    console.log("substring(1,4):", sub1);
    const sub2: string = "abcdef".substring(2);
    console.log("substring(2):", sub2);
}
