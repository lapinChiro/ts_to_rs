import * as fs from 'fs';

function main(): void {
    const input: string = fs.readFileSync("/dev/stdin", "utf8");
    const trimmed: string = input.trim();
    const count: number = trimmed.split("\n").length;
    console.log("count:", count);
    console.log("content:", trimmed);
}
