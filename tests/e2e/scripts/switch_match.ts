function dayName(day: number): string {
    switch (day) {
        case 1:
            return "Monday";
        case 2:
            return "Tuesday";
        case 3:
            return "Wednesday";
        default:
            return "Other";
    }
}

function grade(score: number): string {
    if (score >= 90) {
        return "A";
    } else if (score >= 80) {
        return "B";
    } else if (score >= 70) {
        return "C";
    } else {
        return "F";
    }
}

function main(): void {
    console.log("day 1:", dayName(1));
    console.log("day 2:", dayName(2));
    console.log("day 3:", dayName(3));
    console.log("day 99:", dayName(99));
    console.log("grade 95:", grade(95));
    console.log("grade 85:", grade(85));
    console.log("grade 75:", grade(75));
    console.log("grade 50:", grade(50));
}
