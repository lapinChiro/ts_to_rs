function main(): void {
    console.log("parseInt:", parseInt("42"));
    console.log("parseFloat:", parseFloat("3.14"));

    console.log("isNaN false:", Number.isNaN(42));
    console.log("isFinite true:", Number.isFinite(1));
    console.log("isInteger true:", Number.isInteger(3));
    console.log("isInteger false:", Number.isInteger(3.5));

    // Math operations with parsed values
    const x: number = parseInt("10");
    const y: number = parseFloat("2.5");
    console.log("sum:", x + y);
    console.log("product:", x * y);

    // NaN and Infinity
    console.log("isNaN NaN:", isNaN(NaN));
    console.log("isFinite Infinity:", Number.isFinite(Infinity));
    console.log("NaN equality:", NaN === NaN);
}
