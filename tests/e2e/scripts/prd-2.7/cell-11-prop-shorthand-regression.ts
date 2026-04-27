// PRD 2.7 Matrix Cell #11: Prop::Shorthand × regression lock-in
//
// 既存 Prop::Shorthand (`{ x }` = `{ x: x }`) handle の regression lock-in test。
// PRD 2.7 の `_` arm 削除 + 全 Prop variant explicit enumerate に伴い、既存
// Shorthand handle が破壊されていないことを E2E で verify。

interface Point {
    x: number;
    y: number;
}

function makePoint(x: number, y: number): Point {
    return { x, y };  // Prop::Shorthand (x + y、両方 Shorthand)
}

function main(): void {
    const p = makePoint(3, 4);
    console.log(p.x);  // 3
    console.log(p.y);  // 4
}
