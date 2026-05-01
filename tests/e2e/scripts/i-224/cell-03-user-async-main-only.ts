// Cell 3: A0 + B2 — declarations only + user async main (regression lock-in)
async function main(): Promise<void> { console.log("user async main"); }
