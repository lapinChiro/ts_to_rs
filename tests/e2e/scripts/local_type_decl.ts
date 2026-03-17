function processData(): string {
    // Local type declarations should be silently skipped without error
    interface Config {
        name: string;
        value: number;
    }
    type Status = string;

    const msg: string = "ok";
    return msg;
}

function main(): void {
    console.log("result:", processData());
}
