// Cell 4: A0 + B3 — declarations only + user `main` non-fn symbol (interface)
// Ideal: declarations only emit、no fn main needed (regression lock-in)
interface main { id: number; }
export type Mode = "dev" | "prod";
