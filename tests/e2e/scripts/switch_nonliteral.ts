function describeStatus(status: number): string {
    const STATUS_ACTIVE: number = 1;
    const STATUS_INACTIVE: number = 2;
    const STATUS_PENDING: number = 3;
    switch (status) {
        case STATUS_ACTIVE:
            return "active";
        case STATUS_INACTIVE:
            return "inactive";
        case STATUS_PENDING:
            return "pending";
        default:
            return "unknown";
    }
}

function mixedSwitch(x: number): string {
    const SPECIAL: number = 99;
    switch (x) {
        case 1:
            return "one";
        case 2:
            return "two";
        case SPECIAL:
            return "special";
        default:
            return "other";
    }
}

function main(): void {
    console.log(describeStatus(1));
    console.log(describeStatus(2));
    console.log(describeStatus(3));
    console.log(describeStatus(999));
    console.log(mixedSwitch(1));
    console.log(mixedSwitch(2));
    console.log(mixedSwitch(99));
    console.log(mixedSwitch(50));
}
