import * as fs from 'fs';

function main(): void {
    const dir: string = process.env.TEST_DIR!;
    const path1: string = `${dir}/test.txt`;
    const path2: string = `${dir}/no_such_file.txt`;

    // Write
    fs.writeFileSync(path1, "hello world");
    console.log("wrote file");

    // Read
    const content: string = fs.readFileSync(path1, "utf8");
    console.log("read:", content);

    // Exists
    const exists: boolean = fs.existsSync(path1);
    console.log("exists:", exists);

    // Non-existent
    const missing: boolean = fs.existsSync(path2);
    console.log("missing:", missing);
}
