class Logger { _prefix = "[INFO]"; get prefix(): string { return this._prefix; } log(msg: string): void { console.log(this.prefix + " " + msg); } }
const l = new Logger();
l.log("hello");
