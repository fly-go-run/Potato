export type DiffLineKind = "same" | "remove" | "add";

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
}

export function lineDiff(before: string, after: string): DiffLine[] {
  const oldLines = splitLines(before);
  const newLines = splitLines(after);
  const lengths = Array.from({ length: oldLines.length + 1 }, () =>
    Array<number>(newLines.length + 1).fill(0),
  );

  for (let oldIndex = oldLines.length - 1; oldIndex >= 0; oldIndex -= 1) {
    for (let newIndex = newLines.length - 1; newIndex >= 0; newIndex -= 1) {
      lengths[oldIndex]![newIndex] =
        oldLines[oldIndex] === newLines[newIndex]
          ? lengths[oldIndex + 1]![newIndex + 1]! + 1
          : Math.max(
              lengths[oldIndex + 1]![newIndex]!,
              lengths[oldIndex]![newIndex + 1]!,
            );
    }
  }

  const result: DiffLine[] = [];
  let oldIndex = 0;
  let newIndex = 0;
  while (oldIndex < oldLines.length && newIndex < newLines.length) {
    if (oldLines[oldIndex] === newLines[newIndex]) {
      result.push({ kind: "same", text: oldLines[oldIndex]! });
      oldIndex += 1;
      newIndex += 1;
    } else if (
      lengths[oldIndex + 1]![newIndex]! >=
      lengths[oldIndex]![newIndex + 1]!
    ) {
      result.push({ kind: "remove", text: oldLines[oldIndex]! });
      oldIndex += 1;
    } else {
      result.push({ kind: "add", text: newLines[newIndex]! });
      newIndex += 1;
    }
  }
  while (oldIndex < oldLines.length) {
    result.push({ kind: "remove", text: oldLines[oldIndex]! });
    oldIndex += 1;
  }
  while (newIndex < newLines.length) {
    result.push({ kind: "add", text: newLines[newIndex]! });
    newIndex += 1;
  }
  return result;
}

function splitLines(value: string): string[] {
  return value === "" ? [] : value.split("\n");
}
