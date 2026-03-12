/**
 * Builds bidirectional line-number mappings between original and modified sides
 * of a Monaco diff editor, using the raw lineChanges from getLineChanges().
 *
 * Precomputes lookup maps in a single pass over the hunks so that individual
 * lookups are O(1).
 */
export function buildLineMapping(lineChanges) {
  if (!lineChanges || lineChanges.length === 0) {
    return {
      originalToModified: (line) => line,
      modifiedToOriginal: (line) => line,
    };
  }

  const hunks = lineChanges
    .map(change => ({
      origStart: change.originalStartLineNumber,
      origEnd: change.originalEndLineNumber,
      modStart: change.modifiedStartLineNumber,
      modEnd: change.modifiedEndLineNumber,
    }))
    .sort((a, b) => a.modStart - b.modStart);

  // Precompute both maps in a single walk through the hunks.
  // modToOrig: modified line → original line (or null for pure insertions)
  // origToMod: original line → modified line (or null for lines inside a hunk)
  const modToOrig = new Map();
  const origToMod = new Map();

  let modCursor = 1;
  let origCursor = 1;

  for (const hunk of hunks) {
    const origHunkSize = Math.max(0, hunk.origEnd - hunk.origStart + 1);
    const modHunkSize = Math.max(0, hunk.modEnd - hunk.modStart + 1);

    // Unchanged lines before this hunk — 1:1 mapping
    while (modCursor < hunk.modStart) {
      modToOrig.set(modCursor, origCursor);
      origToMod.set(origCursor, modCursor);
      modCursor++;
      origCursor++;
    }

    // Lines inside the hunk
    // Modified side → original side (positional, clamped)
    for (let i = 0; i < modHunkSize; i++) {
      const modLine = hunk.modStart + i;
      if (origHunkSize === 0) {
        // Pure insertion — no original counterpart
        modToOrig.set(modLine, null);
      } else {
        // Positional mapping, clamped to original range
        modToOrig.set(modLine, Math.min(hunk.origStart + i, hunk.origEnd));
      }
    }

    // Original side → modified side (null — inside a changed region)
    for (let i = 0; i < origHunkSize; i++) {
      origToMod.set(hunk.origStart + i, null);
    }

    modCursor = hunk.modStart + modHunkSize;
    origCursor = hunk.origStart + origHunkSize;
  }

  // Remaining unchanged lines after the last hunk — store the offset so
  // the lookup function can compute them without knowing total line count.
  const tailOffset = origCursor - modCursor;
  const tailModStart = modCursor;
  const tailOrigStart = origCursor;

  return {
    modifiedToOriginal(modLine) {
      if (modToOrig.has(modLine)) return modToOrig.get(modLine);
      // After all hunks — apply the final cumulative offset
      if (modLine >= tailModStart) return modLine + tailOffset;
      return modLine; // shouldn't happen, but safe fallback
    },
    originalToModified(origLine) {
      if (origToMod.has(origLine)) return origToMod.get(origLine);
      if (origLine >= tailOrigStart) return origLine - tailOffset;
      return origLine;
    },
  };
}
