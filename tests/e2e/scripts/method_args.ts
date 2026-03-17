class Logger {
    prefix: string;
    constructor(prefix: string) {
        this.prefix = prefix;
    }
    log(msg: string): void {
        console.log(this.prefix, msg);
    }
}

function main(): void {
    const l: Logger = new Logger("INFO:");
    l.log("started");
    l.log("finished");
}
