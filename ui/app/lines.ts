// A sentence laid in rows of one width: the title's line about its map, on screens 400 to 480 wide.

/**
 * `text` in at most `rows` rows no wider than `width`, as `wide` measures a row: broken between words,
 * and when the words go on past the last row, that row is cut at a word and ends in three dots. A word
 * wider than a row stands alone and runs over: a line is written to have none.
 */
export function fitLines(text: string, width: number, rows: number, wide: (row: string) => number): string[] {
  const words = text.split(" ").filter(Boolean);
  const out: string[] = [];
  let row = "";
  for (const word of words) {
    const longer = row ? `${row} ${word}` : word;
    if (!row || wide(longer) <= width) {
      row = longer;
      continue;
    }
    if (out.length === rows - 1) {
      // The last row, and this word and what follows do not fit: words leave until the dots do.
      let cut = row;
      while (cut.includes(" ") && wide(`${cut}...`) > width) cut = cut.slice(0, cut.lastIndexOf(" "));
      out.push(`${cut.replace(/[,:;]$/, "")}...`);
      return out;
    }
    out.push(row);
    row = word;
  }
  if (row) out.push(row);
  return out;
}
