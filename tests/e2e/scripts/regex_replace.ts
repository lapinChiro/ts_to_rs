function main(): void {
    const s: string = "aaa bbb aaa";

    // Global replace: all occurrences
    const globalResult: string = s.replace(/aaa/g, "ccc");
    console.log("global:", globalResult);

    // Non-global replace: first occurrence only
    const firstResult: string = s.replace(/aaa/, "ccc");
    console.log("first:", firstResult);

    // String replace: first occurrence only (I-172)
    const strResult: string = s.replace("aaa", "ccc");
    console.log("string:", strResult);

    // Global replace with flags
    const mixed: string = "Hello HELLO hello";
    const ciResult: string = mixed.replace(/hello/gi, "hi");
    console.log("case-insensitive global:", ciResult);

    // I-176: regex.test() → is_match()
    const hasDigits: boolean = /\d+/.test("abc123");
    console.log("has digits:", hasDigits);
    const noDigits: boolean = /\d+/.test("abcdef");
    console.log("no digits:", noDigits);
}
