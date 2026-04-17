// Cell 1: FieldAccess × Option<String> × Stmt — verify ??= compiles and runs
interface Config { name?: string; }
function ensureName(c: Config): void {
    c.name ??= "default";
}
function main(): void {
    const c1: Config = {};
    ensureName(c1);
    console.log("fa-option-stmt:ok");
}
