# Plan: Review Any Line at Any Point During Commit Review

## Summary

Currently, line review is constrained to diff change blocks (hunks). Clicking a line in the glyph margin reviews the **entire hunk** it belongs to, and lines outside of diff regions cannot be reviewed at all. This plan changes the behavior so that:

1. **Any line** in the modified file can be reviewed, not just lines in diff hunks.
2. Clicking a single line affects **only that line** (not the whole hunk).
3. **Click-and-drag** selects a contiguous range of lines, then shows the stoplight popup for that range.

## Affected Files

| File | Changes |
|------|---------|
| `src/renderer/pages/review/editor/useLineReviewDecorations.js` | Major rewrite of decoration logic and click/drag handling |
| `src/renderer/pages/review/editor/DiffEditor.jsx` | Pass total line count to hook; minor API changes |
| `src/renderer/pages/review/ReviewContext.jsx` | Update file progress computation to not require change-block coverage |
| `src/renderer/styles.css` | Add drag-selection highlight style |

**No backend changes required.** The backend stores arbitrary `(start, end, state)` ranges without validating they fall within a diff.

## Detailed Changes

### 1. `useLineReviewDecorations.js` — Decoration Logic

**Current:** Iterates only over lines within `changeBlocks`. Each line gets a dot based on: explicit review > file default > unreviewed/assumed-green.

**New:** Iterate over **all lines** in the modified editor model (1 to `model.getLineCount()`).

- Lines **within a change block** that have no explicit review and no file default: show `line-review-dot-unreviewed` (gray dot) in interactive mode, `line-review-dot-green` in read-only mode. (Same as today.)
- Lines **outside any change block** that have no explicit review and no file default: show **no dot** (no decoration). These lines are "clean" and don't need review by default.
- Lines **outside any change block** that **have** an explicit review: show the appropriate colored dot. (This is new — previously impossible.)
- Lines with a file default state: show the default dot regardless of whether they're in a change block. (Same as today for diff lines; new for non-diff lines.)

This means non-diff lines are visually clean by default but will show a dot if the user explicitly reviews them or if a file default is set.

### 2. `useLineReviewDecorations.js` — Click and Drag Interaction

**Current:** Single `onMouseDown` handler. Finds the hunk containing the clicked line and opens the popup for the full hunk range.

**New:** Implement click-and-drag selection:

#### State
- `dragState` ref: `{ isDragging: boolean, startLine: number | null }`
- `selectionRange` state: `{ start: number, end: number } | null` — drives highlight decorations during drag

#### Mouse Events

**`onMouseDown` (glyph margin click):**
1. Record `dragState = { isDragging: true, startLine: lineNumber }`.
2. Set `selectionRange = { start: lineNumber, end: lineNumber }`.
3. Do NOT open popup yet.

**`onMouseMove` (while dragging):**
1. If `dragState.isDragging` and mouse is over a line (any target type with a position), update `selectionRange.end` to that line number.
2. Apply a temporary highlight decoration to the selected range (e.g., a subtle background highlight via `className` on each line in the range).
3. Ensure `start <= end` normalization when applying decorations (user can drag up or down).

**`onMouseUp`:**
1. If `dragState.isDragging`:
   - Finalize `selectionRange` (normalize start/end so start <= end).
   - Look up existing review for that exact range.
   - Open the stoplight popup with `{ position, range: selectionRange, currentState, issueRef, path }`.
   - Reset `dragState`.
   - Clear the temporary highlight decorations (the popup/review dot will take over).
2. If not dragging, ignore.

**Single click:** This naturally falls out of the drag logic — a click without movement produces `start === end`, i.e., a single-line range.

#### Edge Cases
- If the user drags outside the editor, treat `onMouseUp` on `document` as the end of the drag (attach a global listener on mousedown, remove on mouseup).
- If the user presses Escape during drag, cancel the selection.

### 3. `DiffEditor.jsx` — Pass Hook Dependencies

Currently passes `changeBlocks` to the hook. The hook will still receive `changeBlocks` (to know which lines are diff lines for the "unreviewed" dot logic), but the hook no longer uses them to constrain which lines are decoratable or clickable.

No signature change needed — `changeBlocks` is already a parameter and will continue to be used for determining default decoration behavior.

### 4. `ReviewContext.jsx` — File Progress Computation

**Current:** A file is "complete" when every change block is fully covered by a line review range (or a file default is set).

**New behavior:** Keep this logic as-is. The definition of "complete" still means "all diff lines are reviewed." Reviewing non-diff lines is optional and supplementary — it doesn't affect progress tracking. This is the simplest and most correct behavior: you must review all changed lines to complete a file, but you *can* additionally review any other line.

No changes needed to the `SET_LINE_REVIEW` reducer — it already stores arbitrary `(start, end, state)` ranges.

### 5. `styles.css` — Drag Selection Highlight

Add a new CSS class for the temporary drag highlight:

```css
.line-review-drag-highlight {
  background: rgba(100, 150, 255, 0.15);
}
```

This will be applied as a `className` (whole-line decoration) to lines in the selection range during drag.

## Implementation Order

1. **styles.css** — Add the drag highlight class.
2. **useLineReviewDecorations.js** — Rewrite decoration logic to cover all lines + implement click-and-drag.
3. **DiffEditor.jsx** — Adjust if any API changes are needed (likely minimal/none).
4. **Manual testing** — Verify:
   - Single click on a diff line reviews just that line (not the whole hunk).
   - Click-and-drag across multiple lines highlights them and opens popup for that range.
   - Non-diff lines can be clicked and reviewed.
   - File progress still works correctly (all diff hunks must be covered).
   - Existing reviews from backend load and display correctly.
   - Read-only mode (branch review) still works.

## What Does NOT Change

- Backend / Rust code — no validation changes needed.
- Data model — `lineRanges` already supports arbitrary `(start, end)` ranges.
- `ReviewPopup` component — still receives `{ position, range, currentState, issueRef, path }`.
- File-level default review behavior.
- Issue linkage for yellow/red reviews.
- Line number translation across commits.
