// Cell 2: A0 + B1 — declarations only + user sync main (regression lock-in)
function helper(): number { return 7; }
function main(): void { console.log("user main:", helper()); }
