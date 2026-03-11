import { describe, it, expect } from 'vitest';
import { buildLineMapping } from './lineMapping.js';

/**
 * Simulates the dual-gutter decoration logic from useLineReviewDecorations
 * to verify that lineSummary (original-side) entries are correctly mapped
 * and do NOT bleed onto wrong modified-side line numbers.
 */
function computeGutterStates({ lineChanges, lineSummary, sessionReviews, modifiedLineCount }) {
  const mapping = buildLineMapping(lineChanges);

  // Build diff lines set (modified-side)
  const diffLines = new Set();
  if (lineChanges) {
    lineChanges.forEach(change => {
      if (change.modifiedStartLineNumber <= change.modifiedEndLineNumber) {
        for (let l = change.modifiedStartLineNumber; l <= change.modifiedEndLineNumber; l++) {
          diffLines.add(l);
        }
      }
    });
  }

  // Build prior review map (original-side line numbers)
  const priorReviewOriginal = new Map();
  if (lineSummary) {
    lineSummary.forEach(entry => {
      for (let l = entry.start; l <= entry.end; l++) {
        priorReviewOriginal.set(l, entry.state);
      }
    });
  }

  // Build session reviews map (modified-side line numbers)
  const reviewedLines = new Map();
  if (sessionReviews) {
    sessionReviews.forEach(range => {
      for (let l = range.start; l <= range.end; l++) {
        reviewedLines.set(l, range.state);
      }
    });
  }

  const results = [];
  for (let line = 1; line <= modifiedLineCount; line++) {
    const origLine = mapping.modifiedToOriginal(line);

    // Before state
    let beforeState;
    if (origLine === null) {
      beforeState = 'none';
    } else {
      beforeState = priorReviewOriginal.get(origLine) || 'green';
    }

    // New state
    let newState;
    const sessionState = reviewedLines.get(line);
    if (sessionState) {
      newState = sessionState;
    } else if (diffLines.has(line)) {
      newState = 'grey';
    } else {
      newState = beforeState;
    }

    results.push({ line, beforeState, newState });
  }
  return results;
}

describe('buildLineMapping', () => {
  it('returns identity mapping when no changes', () => {
    const mapping = buildLineMapping(null);
    expect(mapping.modifiedToOriginal(1)).toBe(1);
    expect(mapping.originalToModified(1)).toBe(1);
    expect(mapping.modifiedToOriginal(50)).toBe(50);
  });

  it('returns identity mapping for empty array', () => {
    const mapping = buildLineMapping([]);
    expect(mapping.modifiedToOriginal(10)).toBe(10);
    expect(mapping.originalToModified(10)).toBe(10);
  });

  it('handles pure insertion (5 new lines inserted before line 10)', () => {
    // Original: lines 1-20
    // Modified: lines 1-9 unchanged, lines 10-14 inserted, lines 15-25 = original 10-20
    const lineChanges = [{
      originalStartLineNumber: 10,
      originalEndLineNumber: 9, // 0-length on original side = pure insertion
      modifiedStartLineNumber: 10,
      modifiedEndLineNumber: 14,
    }];
    const mapping = buildLineMapping(lineChanges);

    // Lines before the insertion map 1:1
    expect(mapping.modifiedToOriginal(1)).toBe(1);
    expect(mapping.modifiedToOriginal(9)).toBe(9);

    // Inserted lines have no original counterpart
    expect(mapping.modifiedToOriginal(10)).toBeNull();
    expect(mapping.modifiedToOriginal(14)).toBeNull();

    // Lines after insertion are offset by 5
    expect(mapping.modifiedToOriginal(15)).toBe(10);
    expect(mapping.modifiedToOriginal(20)).toBe(15);
    expect(mapping.modifiedToOriginal(25)).toBe(20);
  });

  it('handles pure deletion', () => {
    // Original lines 5-9 deleted
    const lineChanges = [{
      originalStartLineNumber: 5,
      originalEndLineNumber: 9,
      modifiedStartLineNumber: 5,
      modifiedEndLineNumber: 4, // 0-length on modified side = pure deletion
    }];
    const mapping = buildLineMapping(lineChanges);

    expect(mapping.modifiedToOriginal(4)).toBe(4);
    // Line 5 on modified = line 10 on original (5 lines deleted)
    expect(mapping.modifiedToOriginal(5)).toBe(10);

    expect(mapping.originalToModified(4)).toBe(4);
    // Deleted lines have no modified counterpart
    expect(mapping.originalToModified(5)).toBeNull();
    expect(mapping.originalToModified(9)).toBeNull();
    expect(mapping.originalToModified(10)).toBe(5);
  });

  it('handles modification (same range size, lines changed)', () => {
    // Lines 5-7 modified (3 lines replaced with 3 lines)
    const lineChanges = [{
      originalStartLineNumber: 5,
      originalEndLineNumber: 7,
      modifiedStartLineNumber: 5,
      modifiedEndLineNumber: 7,
    }];
    const mapping = buildLineMapping(lineChanges);

    expect(mapping.modifiedToOriginal(4)).toBe(4);
    expect(mapping.modifiedToOriginal(5)).toBeNull();
    expect(mapping.modifiedToOriginal(7)).toBeNull();
    expect(mapping.modifiedToOriginal(8)).toBe(8); // no offset change
  });
});

describe('dual-gutter decoration: insertion before reviewed lines', () => {
  // Reproduces the reported bug:
  // - lineSummary has yellow state for original lines 10-18
  // - A change inserts 5 new lines before original line 10,
  //   plus 5 lines replace within the range
  // - Expected: before=yellow appears on modified lines 20-28 (the shifted originals),
  //   NOT on modified lines 10-18
  // - Bug: original-side line numbers 10-18 were incorrectly applied
  //   to modified-side lines 10-18

  const lineChanges = [{
    // 5 lines inserted before original line 10, plus 5 lines modified in-place
    // Original lines 10-14 become modified lines 10-19 (10 new/changed lines)
    originalStartLineNumber: 10,
    originalEndLineNumber: 14,
    modifiedStartLineNumber: 10,
    modifiedEndLineNumber: 19,
  }];

  const lineSummary = [
    { start: 10, end: 18, state: 'yellow' },
  ];

  it('should NOT show yellow on modified lines 10-18 (those are in the diff hunk or shifted)', () => {
    const states = computeGutterStates({
      lineChanges,
      lineSummary,
      sessionReviews: [],
      modifiedLineCount: 30,
    });

    // Modified lines 10-19 are inside the diff hunk
    for (let l = 10; l <= 19; l++) {
      const s = states.find(s => s.line === l);
      expect(s.beforeState).toBe('none'); // no original counterpart (in hunk)
      expect(s.newState).toBe('grey');    // unreviewed diff line
    }

    // Modified lines 20-23 correspond to original 15-18 (which are yellow in lineSummary)
    for (let l = 20; l <= 23; l++) {
      const s = states.find(s => s.line === l);
      expect(s.beforeState).toBe('yellow');
      expect(s.newState).toBe('yellow'); // carries forward (not in diff hunk)
    }

    // Modified line 24 corresponds to original 19 (no lineSummary entry → green)
    const s24 = states.find(s => s.line === 24);
    expect(s24.beforeState).toBe('green');
    expect(s24.newState).toBe('green');
  });

  it('should show yellow before-state on the correctly shifted lines', () => {
    const states = computeGutterStates({
      lineChanges,
      lineSummary,
      sessionReviews: [],
      modifiedLineCount: 30,
    });

    // Lines before the hunk are unchanged
    for (let l = 1; l <= 9; l++) {
      const s = states.find(s => s.line === l);
      expect(s.beforeState).toBe('green'); // no prior review = assumed green
      expect(s.newState).toBe('green');
    }
  });
});
