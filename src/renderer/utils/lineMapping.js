/**
 * Builds bidirectional line-number mappings between original and modified sides
 * of a Monaco diff editor, using the raw lineChanges from getLineChanges().
 *
 * For unchanged lines (outside any diff hunk), computes a cumulative offset
 * from insertions/deletions in prior hunks.
 * For lines within a modification hunk (both sides have lines), maps
 * positionally within the hunk, clamped to the original range.
 * For pure insertions (no original lines), returns null.
 */
export function buildLineMapping(lineChanges) {
  if (!lineChanges || lineChanges.length === 0) {
    // No changes — every line maps 1:1
    return {
      originalToModified: (line) => line,
      modifiedToOriginal: (line) => line,
    };
  }

  // Build sorted list of hunks with both sides
  const hunks = lineChanges
    .map(change => ({
      origStart: change.originalStartLineNumber,
      origEnd: change.originalEndLineNumber,
      modStart: change.modifiedStartLineNumber,
      modEnd: change.modifiedEndLineNumber,
    }))
    .sort((a, b) => a.modStart - b.modStart);

  /**
   * Map a modified-side line number to the corresponding original-side line number.
   * For unchanged lines: computes exact mapping via cumulative offset.
   * For modification hunks (both sides have lines): positional mapping, clamped.
   * For pure insertions (no original lines): returns null.
   */
  function modifiedToOriginal(modLine) {
    let offset = 0;

    for (const hunk of hunks) {
      if (modLine < hunk.modStart) {
        return modLine + offset;
      }

      const modHunkSize = Math.max(0, hunk.modEnd - hunk.modStart + 1);
      const origHunkSize = Math.max(0, hunk.origEnd - hunk.origStart + 1);

      if (hunk.modEnd >= hunk.modStart && modLine >= hunk.modStart && modLine <= hunk.modEnd) {
        // Inside the hunk on the modified side
        if (origHunkSize === 0) {
          // Pure insertion — no original lines to map to
          return null;
        }
        // Positional mapping within the hunk, clamped to original range
        const posInHunk = modLine - hunk.modStart;
        return Math.min(hunk.origStart + posInHunk, hunk.origEnd);
      }

      offset += origHunkSize - modHunkSize;
    }

    return modLine + offset;
  }

  /**
   * Map an original-side line number to the corresponding modified-side line number.
   * Returns null if the line is inside a diff hunk (no 1:1 mapping).
   */
  function originalToModified(origLine) {
    let offset = 0; // cumulative: modified = original + offset

    for (const hunk of hunks) {
      const origHunkSize = Math.max(0, hunk.origEnd - hunk.origStart + 1);
      const modHunkSize = Math.max(0, hunk.modEnd - hunk.modStart + 1);

      // Before this hunk on the original side
      if (origLine < hunk.origStart) {
        return origLine + offset;
      }

      // Inside the original side of this hunk
      if (hunk.origEnd >= hunk.origStart && origLine >= hunk.origStart && origLine <= hunk.origEnd) {
        return null; // inside a changed region
      }

      // Past this hunk — accumulate offset
      offset += modHunkSize - origHunkSize;
    }

    // After all hunks
    return origLine + offset;
  }

  return { originalToModified, modifiedToOriginal };
}
