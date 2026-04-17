// Cell 2: FieldAccess × Option<String> × Expr — expression-context ??=
interface Config { name?: string; }
function getOrSetName(c: Config): string {
    return (c.name ??= "default");
}
function main(): void {
    console.log("fa-option-expr:" + getOrSetName({}));
    console.log("fa-option-expr:" + getOrSetName({ name: "existing" }));
}
