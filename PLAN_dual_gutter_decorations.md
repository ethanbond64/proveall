# Plan: Dual-Column Gutter Decorations + Issue Conflict Prompting

## Summary

Replace the single-column gutter dots with two columns:
- **Left column ("Before")**: Shows the review state from the previous commit/event, aligned to the **original (left-side)** editor line numbers.
- **Right column ("New")**: Shows the new review state the user is building in this session, aligned to the **modified (right-side)** editor line numbers.

Then add modal prompting when a user approves a line that previously had outstanding issues.

---

## Key Findings from Codebase Investigation

1. **Backend `line_summary` only contains data for lines that were explicitly reviewed** — it stores `{start, end, state, issue_id}` entries from `CompositeFileReviewState.summary_metadata`. Lines not covered by any entry have no prior state (i.e., they are implicitly "green" / never-reviewed).

2. **`changeBlocks` currently only stores modified-side line numbers** (`modifiedStartLineNumber`, `modifiedEndLineNumber`). We will also need original-side line numbers for the "Before" column.

3. **Monaco's `getLineChanges()` already returns both sides**: `originalStartLineNumber`, `originalEndLineNumber`, `modifiedStartLineNumber`, `modifiedEndLineNumber`. We just need to capture and pass through the original-side numbers too.

4. **The hook currently decorates only the modified editor** (`diffEditor.getModifiedEditor()`). For the "Before" column, we'll need to also decorate the **original editor** (`diffEditor.getOriginalEditor()`).

5. **`lineSummary` line numbers from the backend correspond to the state at the HEAD of the branch context** (i.e., the cumulative review state). These need to be mapped to original-side line numbers for the "Before" column.

<!-- TODO: Confirm with Ethan — do the lineSummary line numbers from the backend correspond to
     the file content at the HEAD event's commit? Or at some other ref? This matters for mapping
     them to the original-side editor. My assumption is they correspond to the content at the
     commit that was reviewed (the HEAD event's commit), which would be the "original" side
     content when reviewing a new commit on top of it. -->

<!-- TODO: Confirm — when reviewing a range of commits (not just one), should the "Before"
     column show the state from the commit just before the range, or from the earliest commit
     in the range? The user said "earliest commit in the range" but that seems like it would
     show state from BEFORE the range, not from the earliest commit IN the range. Clarify. -->

---

## Part 1: Dual-Column Gutter UI (No Prompting Logic)

### Step 1.1: Capture original-side change blocks in DiffEditor

**File:** `src/renderer/pages/review/editor/DiffEditor.jsx`

- In `computeChangeBlocks()`, also capture `originalStartLineNumber` and `originalEndLineNumber` from `getLineChanges()`.
- Store them alongside modified blocks, e.g.:
  ```js
  blocks.push({
    startLine: change.modifiedStartLineNumber,
    endLine: change.modifiedEndLineNumber,
    originalStartLine: change.originalStartLineNumber,
    originalEndLine: change.originalEndLineNumber,
  });
  ```
- Pass the original editor ref (`diffEditor.getOriginalEditor()`) down alongside the modified editor ref.

### Step 1.2: Expose the original editor from DiffEditor

**File:** `src/renderer/pages/review/editor/DiffEditor.jsx`

- Add a new ref: `originalEditorRef` alongside `modifiedEditorRef`.
- Set it in the content update effect: `originalEditorRef.current = diffEditorRef.current.getOriginalEditor();`
- The "Before" decorations hook will need this editor instance.

### Step 1.3: Create `useBeforeDecorations` hook (left-column / original editor)

**New file:** `src/renderer/pages/review/editor/useBeforeDecorations.js`

This hook decorates the **original (left-side) editor** with read-only dots representing the prior review state.

**Inputs:**
- `editor` — the original editor instance
- `lineSummary` — the backend-provided `{start, end, state, issue_id}[]` from the previous event
- `changeBlocks` — with original-side line numbers (to know which lines are in the diff)

**Behavior:**
- For each line in the original editor:
  - If covered by a `lineSummary` entry → show that state's color dot
  - If NOT covered by any entry → show **green** dot (default: no outstanding issues)
- These dots are **read-only** (no click handling, no selection highlight)
- Hover message: `"Previous review: {state}"` or `"No previous review (assumed approved)"`

**CSS classes:** Reuse existing `line-review-dot-{red,yellow,green}` classes. Add a subtle visual distinction (e.g., slightly smaller or with lower opacity) to differentiate "Before" dots from "New" dots — via a wrapper class like `before-dot`.

### Step 1.4: Update `useLineReviewDecorations` for right-column behavior

**File:** `src/renderer/pages/review/editor/useLineReviewDecorations.js`

Modify the existing decoration logic for the **modified (right-side) editor**:

**New default behavior for non-diff, non-reviewed lines:**
- Currently: gray dot (`line-review-dot-unreviewed`) or no dot
- **New:** Green dot if the line has no prior state, OR carry forward the prior state's color if the line existed before and had a non-green review.

<!-- TODO: To "carry forward" prior state for non-diff lines, we need to map original-side
     lineSummary entries to modified-side line numbers. This requires using the line change
     mappings from getLineChanges() to compute the offset. Lines that are unchanged between
     original and modified will have a direct 1:1 mapping (adjusted for insertions/deletions
     above them). Need to build a line-number translation function. -->

**Updated priority for the right column:**
1. **Explicitly reviewed in this session** → show that state's color
2. **In diff hunk, not yet reviewed** → `unreviewed-required` (gray, interactive) — same as current
3. **Not in diff hunk, has prior state from lineSummary (mapped to modified line numbers)** → carry forward that color (read-only unless user clicks to re-review)
4. **Not in diff hunk, no prior state** → green dot

**New input needed:** `lineSummary` (from backend) and the line-number mapping from original→modified side.

### Step 1.5: Build line-number translation utility

**New file:** `src/renderer/utils/lineMapping.js`

Given Monaco's `lineChanges` array (from `getLineChanges()`), build a function that maps an original-side line number to a modified-side line number (and vice versa). This handles:
- Lines before any diff hunk: offset = 0
- Lines between hunks: offset = cumulative inserted - deleted lines from prior hunks
- Lines within a hunk: no direct mapping (line was changed)

```js
export function buildLineMapping(lineChanges) {
  // Returns { originalToModified(line), modifiedToOriginal(line) }
}
```

### Step 1.6: Pass `lineSummary` and original editor to hooks

**File:** `src/renderer/pages/review/editor/DiffEditor.jsx`

- Pass `lineSummary` from `openFiles.get(path)` down to both hooks.
- Instantiate `useBeforeDecorations(originalEditorRef.current, lineSummary, changeBlocks)`.
- Pass `lineSummary` and line mapping to the updated `useLineReviewDecorations`.

### Step 1.7: CSS updates for dual-column appearance

**File:** `src/renderer/styles.css`

- Ensure both editors in the diff view have `glyphMargin: true` (original editor may need this explicitly).
- Add `.before-dot` modifier class: slightly reduced opacity (0.7) or size to visually distinguish from the active "New" column.
- The two columns are naturally separated because they live on different editors (original vs modified) in side-by-side mode.

<!-- TODO: In inline diff mode (not side-by-side), both columns would need to appear on the
     same editor. This is significantly more complex with Monaco's glyph margin (only one
     glyph per line). Options:
     1. Disable dual dots in inline mode (just show new-state dots)
     2. Use margin decorations instead of glyph decorations for one column
     3. Force side-by-side mode when dual dots are active
     Recommend option 3 (force side-by-side) for simplicity. -->

### Step 1.8: Handle the `glyphMargin` on the original editor

**File:** `src/renderer/pages/review/editor/DiffEditor.jsx`

Monaco diff editor's original side may not have `glyphMargin` enabled by default. Check and ensure:
```js
const diffEditor = monaco.editor.createDiffEditor(containerRef.current, {
  // ... existing options ...
  originalEditable: false,
  // The original editor options may need to be set separately:
});
// After creation:
diffEditor.getOriginalEditor().updateOptions({ glyphMargin: true });
```

---

## Part 2: Issue Conflict Prompting Modal

When the user marks a line as **green/approved** on the right column, and that line was **non-green** in the "Before" column (meaning it has outstanding issues), show a confirmation modal.

### Step 2.1: Create `IssueConflictModal` component

**New file:** `src/renderer/pages/review/components/IssueConflictModal.jsx`

**Props:**
- `issues` — array of `{id, comment, state}` objects for all outstanding issues tied to the lines being approved
- `onResolve(issueId)` — resolve the issue
- `onKeepOpenApprove(issueId)` — keep issue open but mark line green
- `onMaintain(issueId)` — keep issue AND keep line at its prior state (cancel the green for this issue's lines)
- `onClose` — close modal (cancel all)

**UI:**
- Backdrop overlay (like `IssueResolveConfirmation`)
- Stack of issue cards (one per issue in the hunk being approved)
- Each card shows:
  - Issue ID (first 8 chars) and comment
  - Message: "You are approving a line that was previously tied to this issue:"
  - Three buttons:
    1. **"Resolve Issue"** — marks issue as resolved, line goes green
    2. **"Keep Issue Open, Approve Line"** — line goes green, issue stays open
    3. **"Keep Issue & Line State"** — line stays at prior color, issue untouched
- User addresses each issue card one at a time (stack behavior — after addressing one, the next appears)
- When all issues are addressed, modal closes and all decisions are applied

### Step 2.2: Detect issue conflicts on green click

**File:** `src/renderer/pages/review/editor/useLineReviewDecorations.js` (or a new helper)

When the user clicks green in the `ReviewPopup`:
1. Check if any line in the selected range had a non-green state in `lineSummary`
2. If yes, collect all `issue_id` values from those `lineSummary` entries
3. Look up issue details from `context.issues` or `openFiles.get(path).issues`
4. If there are conflicting issues → open `IssueConflictModal` instead of immediately saving
5. If no conflicts → save immediately (current behavior)

### Step 2.3: Wire `IssueConflictModal` into `ReviewPopup`

**File:** `src/renderer/pages/review/components/ReviewPopup.jsx`

- Add state: `const [issueConflicts, setIssueConflicts] = useState(null)`
- Modify `handleGreenClick`:
  ```js
  const handleGreenClick = () => {
    const conflicts = detectIssueConflicts(range, lineSummary, issues);
    if (conflicts.length > 0) {
      setIssueConflicts(conflicts);
      return; // Don't save yet — modal will handle it
    }
    // No conflicts — save immediately (existing behavior)
    // ...
  };
  ```
- Render `IssueConflictModal` when `issueConflicts` is non-null

### Step 2.4: Implement conflict resolution handlers

**File:** `src/renderer/pages/review/components/ReviewPopup.jsx` (or `IssueConflictModal`)

For each issue the user addresses:

1. **"Resolve Issue"**:
   - Call `context.actions.resolveIssue(issueId)` (mark as resolved)
   - Proceed to mark lines green

2. **"Keep Issue Open, Approve Line"**:
   - Do NOT resolve the issue
   - Mark lines green anyway (the new review state overrides)

3. **"Keep Issue & Line State"**:
   - Do NOT resolve the issue
   - Do NOT mark those specific lines green — they carry forward their prior state
   - If the user's selection covered multiple issues and they chose differently for each, the final `updateLineReview` call needs to account for partial ranges

<!-- TODO: The "partial range" scenario is complex. If a user selects lines 10-30 and
     lines 10-15 had issue A (red) and lines 20-25 had issue B (yellow), and they
     choose "Resolve" for A but "Maintain" for B, we'd need to:
     - Mark lines 10-15 green
     - Mark lines 16-19 green (no prior issues)
     - Leave lines 20-25 at yellow
     - Mark lines 26-30 green (no prior issues)
     This means splitting the original range into sub-ranges based on conflict resolution
     decisions. Need to implement range-splitting logic. -->

### Step 2.5: Add `IssueConflictModal` CSS styles

**File:** `src/renderer/styles.css`

- Backdrop: same pattern as `.confirmation-backdrop`
- Card stack: vertical stack with subtle offset/shadow to indicate depth
- Three action buttons styled distinctly:
  - Resolve: green-tinted
  - Keep Open + Approve: yellow-tinted
  - Maintain: neutral/gray
- Transition animation as cards are resolved (slide out / fade)

### Step 2.6: Pass `lineSummary` and issues data through to `ReviewPopup`

**Files:**
- `src/renderer/pages/review/editor/DiffEditor.jsx` — pass `lineSummary` and `issues` as props or via context
- `src/renderer/pages/review/components/ReviewPopup.jsx` — accept and use these new props

The `lineSummary` data is already available in `openFiles.get(path)` in the ReviewContext, so the popup can access it via `useReviewContext()`.

---

## File Change Summary

| File | Action | Part |
|------|--------|------|
| `src/renderer/pages/review/editor/DiffEditor.jsx` | Modify | 1 |
| `src/renderer/pages/review/editor/useLineReviewDecorations.js` | Modify | 1 & 2 |
| `src/renderer/pages/review/editor/useBeforeDecorations.js` | **Create** | 1 |
| `src/renderer/utils/lineMapping.js` | **Create** | 1 |
| `src/renderer/styles.css` | Modify | 1 & 2 |
| `src/renderer/pages/review/components/ReviewPopup.jsx` | Modify | 2 |
| `src/renderer/pages/review/components/IssueConflictModal.jsx` | **Create** | 2 |

---

## Open Questions (TODOs)

1. **LineSummary line number reference frame**: Do the `start`/`end` values in `lineSummary` correspond to the file at the HEAD event's commit? (Assumed yes — this would mean they map to the original-side editor content for a new commit review.)

2. **"Earliest commit in range" clarification**: When reviewing bulk commits, does "before" mean the state from before the entire range, or from the first commit in the range?

3. **Inline diff mode**: Should we force side-by-side mode when dual dots are active, or attempt to show both columns in inline mode? (Recommended: force side-by-side.)

4. **Partial range splitting**: When a user approves a range but chooses "Maintain" for some issues within it, we need range-splitting logic. Confirm this is the desired behavior vs. simpler alternatives (e.g., all-or-nothing per range).
