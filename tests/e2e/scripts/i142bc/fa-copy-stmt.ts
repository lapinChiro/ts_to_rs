// Cell 3: FieldAccess × Option<f64> × Stmt (Copy inner) — verify ??= compiles
interface Stats { count?: number; }
function ensureCount(s: Stats): void {
    s.count ??= 0;
}
function main(): void {
    const s: Stats = {};
    ensureCount(s);
    console.log("fa-copy-stmt:ok");
}
