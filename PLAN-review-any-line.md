# Plan: Review Any Line at Any Point During Commit Review

## Summary

Currently, line review is constrained to diff change blocks (hunks). Clicking a line in the glyph margin reviews the **entire hunk** it belongs to, and lines outside of diff regions cannot be reviewed at all. This plan changes the behavior so that:

1. **Any line** in the modified file can be reviewed, not just lines in diff hunks.
2. Clicking a single line affects **only that line** (not the whole hunk).
3. **Click-and-drag** selects a contiguous range of lines, then shows the stoplight popup for that range.
4. **Shift-click** extends a selection from the last-clicked line to the shift-clicked line, opening the popup for the full inclusive range.

## Affected Files

| File | Changes |
|------|---------|
| `src/renderer/pages/review/editor/useLineReviewDecorations.js` | Major rewrite of decoration logic and click/drag handling |
| `src/renderer/pages/review/editor/DiffEditor.jsx` | Pass total line count to hook; minor API changes |
| `src/renderer/pages/review/ReviewContext.jsx` | No changes needed — progress logic already correct |
| `src/renderer/styles.css` | Add drag-selection highlight style + required-unreviewed dot style |

**No backend changes required.** The backend stores arbitrary `(start, end, state)` ranges without validating they fall within a diff.

## Detailed Changes

### 1. `useLineReviewDecorations.js` — Decoration Logic

**Current:** Iterates only over lines within `changeBlocks`. Each line gets a dot based on: explicit review > file default > unreviewed/assumed-green.

**New:** Iterate over **all lines** in the modified editor model (1 to `model.getLineCount()`).

- Lines **within a change block** that have no explicit review and no file default: show `line-review-dot-unreviewed-required` — a gray dot with an **orange outline** indicating this line is part of the diff and **must** be reviewed to complete the file. In read-only mode, show `line-review-dot-green`. (Visual change from today's plain gray dot.)
- Lines **outside any change block** that have no explicit review and no file default: show `line-review-dot-unreviewed` — a plain gray dot (no orange outline). These lines are optional to review. They show a dot so the user knows they *can* click to review, but the lack of orange signals they aren't required.
- Lines **outside any change block** that **have** an explicit review: show the appropriate colored dot (`line-review-dot-{state}`). (This is new — previously impossible.)
- Lines with a file default state: show the default dot regardless of whether they're in a change block. (Same as today for diff lines; new for non-diff lines.)

This creates a clear visual hierarchy:
- **Orange-outlined gray dot** = diff line, must review (required)
- **Plain gray dot** = non-diff line, can review (optional)
- **Colored dot (green/yellow/red)** = already reviewed

### 2. `useLineReviewDecorations.js` — Click and Drag Interaction

**Current:** Single `onMouseDown` handler. Finds the hunk containing the clicked line and opens the popup for the full hunk range.

**New:** Implement click-and-drag selection:

#### State
- `dragState` ref: `{ isDragging: boolean, startLine: number | null }`
- `lastClickedLine` ref: `number | null` — persists the last line the user clicked (for shift-click). Updated on every mousedown that initiates a selection. Reset when the popup is dismissed.
- `selectionRange` state: `{ start: number, end: number } | null` — drives highlight decorations during drag

#### Mouse Events

**`onMouseDown` (glyph margin click):**
1. **If shift is held** and `lastClickedLine` is set:
   - Compute range from `lastClickedLine` to clicked `lineNumber` (normalize so start <= end).
   - Set `selectionRange` to that range.
   - Immediately open the stoplight popup for that range (same as drag-end behavior).
   - Do NOT update `lastClickedLine` (keep the original anchor so the user can shift-click again to adjust).
   - Return early — do not start a drag.
2. **Otherwise (no shift):**
   - Record `dragState = { isDragging: true, startLine: lineNumber }`.
   - Set `lastClickedLine = lineNumber`.
   - Set `selectionRange = { start: lineNumber, end: lineNumber }`.
   - Do NOT open popup yet (wait for mouseup).

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

**Shift-click:** The user clicks line 10 (popup opens for line 10). They dismiss or ignore it, then shift-click line 20. The popup opens for lines 10–20. They can shift-click line 15 instead to adjust to 10–15. `lastClickedLine` stays at 10 as the anchor until a non-shift click resets it.

#### Edge Cases
- If the user drags outside the editor, treat `onMouseUp` on `document` as the end of the drag (attach a global listener on mousedown, remove on mouseup).
- If the user presses Escape during drag, cancel the selection.
- `lastClickedLine` is cleared when the popup is dismissed without saving, so stale anchors don't persist across unrelated interactions.

### 3. `DiffEditor.jsx` — Pass Hook Dependencies

Currently passes `changeBlocks` to the hook. The hook will still receive `changeBlocks` (to know which lines are diff lines for the "unreviewed" dot logic), but the hook no longer uses them to constrain which lines are decoratable or clickable.

No signature change needed — `changeBlocks` is already a parameter and will continue to be used for determining default decoration behavior.

### 4. `ReviewContext.jsx` — File Progress Computation

**No changes needed.** The existing logic is already correct for the new behavior:

- A file is "complete" when every change block is fully covered by line review ranges (or a file default is set). This remains the requirement — **all diff lines must be reviewed**.
- Reviews on non-diff lines are supplementary. They get stored and displayed but don't affect progress.
- The `SET_LINE_REVIEW` reducer already stores arbitrary `(start, end, state)` ranges without validating they fall within change blocks.

The progress computation checks `changeBlocks` coverage, not total file line coverage, so it naturally ignores non-diff line reviews when computing completeness.

### 5. `styles.css` — New Styles

#### Required-unreviewed dot (orange outline)

Add a new class for unreviewed diff lines that distinguishes them from optional non-diff lines:

```css
.line-review-dot-unreviewed-required {
  background-color: #666666 !important;
  border: 2px solid #ff9800 !important; /* orange outline */
}

.line-review-dot-unreviewed-required:hover {
  background-color: #888888 !important;
  border: 2px solid #ffb74d !important; /* lighter orange on hover */
}
```

The existing `.line-review-dot-unreviewed` (plain gray, no orange outline) is reused for optional non-diff lines. Its current `border: 1px solid rgba(255, 255, 255, 0.1)` is subtle enough to read as "optional."

Both classes share the base dot sizing from the shared selector, which should be updated to include the new class:

```css
.line-review-dot-red,
.line-review-dot-yellow,
.line-review-dot-green,
.line-review-dot-unreviewed,
.line-review-dot-unreviewed-required {
  /* existing shared styles */
}
```

#### Drag selection highlight

```css
.line-review-drag-highlight {
  background: rgba(100, 150, 255, 0.15);
}
```

Applied as a `className` (whole-line decoration) to lines in the selection range during drag.

## Implementation Order

1. **styles.css** — Add the `unreviewed-required` dot class, update shared selector, add drag highlight class.
2. **useLineReviewDecorations.js** — Rewrite decoration logic to cover all lines (using `unreviewed-required` for diff lines, `unreviewed` for non-diff lines) + implement click-and-drag + shift-click.
3. **DiffEditor.jsx** — Adjust if any API changes are needed (likely minimal/none).
4. **Manual testing** — Verify:
   - Single click on a diff line reviews just that line (not the whole hunk).
   - Click-and-drag across multiple lines highlights them and opens popup for that range.
   - Shift-click after a regular click selects the inclusive range between the two lines.
   - Shift-click again adjusts the range end while keeping the original anchor.
   - Non-diff lines show plain gray dots (no orange outline) and can be clicked and reviewed.
   - Diff lines that haven't been reviewed show gray dots with orange outlines.
   - Orange outlines disappear once a diff line is reviewed (replaced by green/yellow/red dot).
   - File progress still works correctly (all diff lines must be covered; non-diff reviews don't affect progress).
   - Existing reviews from backend load and display correctly.
   - Read-only mode (branch review) still works.

## What Does NOT Change

- Backend / Rust code — no validation changes needed.
- Data model — `lineRanges` already supports arbitrary `(start, end)` ranges.
- `ReviewPopup` component — still receives `{ position, range, currentState, issueRef, path }`.
- File-level default review behavior.
- Issue linkage for yellow/red reviews.
- Line number translation across commits.
