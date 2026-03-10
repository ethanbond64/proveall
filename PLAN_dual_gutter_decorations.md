# Plan: Dual-Column Gutter Decorations + Issue Conflict Prompting

## Summary

Replace the single-column gutter dots with two columns, **both in the center gutter** (the modified/right-side editor's glyph margin area):
- **Left dot ("Before")**: Shows the review state from the previous commit/event. Uses Monaco's line-mapping to determine which original-side lines correspond to each modified-side line.
- **Right dot ("New")**: Shows the new review state the user is building in this session.

Then add modal prompting when a user approves a line that previously had outstanding issues.

---

## Key Findings from Codebase Investigation

1. **Backend `line_summary` only contains data for lines that were explicitly reviewed** — it stores `{start, end, state, issue_id}` entries from `CompositeFileReviewState.summary_metadata`. Lines not covered by any entry have no prior state (i.e., they are implicitly "green" / never-reviewed). **The backend stores review state using original-side (pre-diff) line numbers.**

2. **Monaco's `getLineChanges()` returns both sides**: `originalStartLineNumber`, `originalEndLineNumber`, `modifiedStartLineNumber`, `modifiedEndLineNumber`. We can use this to map original-side `lineSummary` entries to modified-side line numbers for unchanged lines, so both dot columns render in the center gutter.

3. **Both dot columns live on the modified (right-side) editor's glyph margin.** Monaco's `glyphMarginClassName` supports multiple CSS classes, so we can render two dots per line using CSS (e.g., a container with two inline-block circles via `::before` and `::after` pseudo-elements, or two stacked glyph margin classes).

4. **Default states differ by column:**
   - **Before column**: Green for lines with no prior review state (assumed approved).
   - **New column**: Grey/empty for lines that are **in a diff hunk** (unreviewed). For lines **not in a diff hunk**, carry forward the prior state from the Before column.

<!-- TODO: Confirm with Ethan — do the lineSummary line numbers from the backend correspond to
     the file content at the HEAD event's commit? Or at some other ref? My assumption is they
     correspond to the content at the commit that was reviewed (the HEAD event's commit), which
     would be the "original" side content when reviewing a new commit on top of it. -->

<!-- TODO: Confirm — when reviewing a range of commits (not just one), should the "Before"
     column show the state from the commit just before the range, or from the earliest commit
     in the range? The user said "earliest commit in the range" but that seems like it would
     show state from BEFORE the range, not from the earliest commit IN the range. Clarify. -->

---

## Part 1: Dual-Column Gutter UI (No Prompting Logic)

Both dot columns render in the **center gutter** — the modified (right-side) editor's glyph margin. Each line gets a single `glyphMarginClassName` that encodes both states, and CSS renders two dots side-by-side using pseudo-elements.

### Step 1.1: Capture full line-change data in DiffEditor

**File:** `src/renderer/pages/review/editor/DiffEditor.jsx`

- In `computeChangeBlocks()`, capture both sides from `getLineChanges()`:
  ```js
  blocks.push({
    startLine: change.modifiedStartLineNumber,
    endLine: change.modifiedEndLineNumber,
    originalStartLine: change.originalStartLineNumber,
    originalEndLine: change.originalEndLineNumber,
  });
  ```
- Store the raw `lineChanges` array (or the enriched blocks) so the line-mapping utility can use it.

### Step 1.2: Build line-number translation utility

**New file:** `src/renderer/utils/lineMapping.js`

Uses the `lineChanges` from Monaco's diff editor to map original-side line numbers to modified-side line numbers for **unchanged lines only** (lines within a diff hunk have no 1:1 mapping).

```js
export function buildLineMapping(lineChanges) {
  // Returns { originalToModified(line), modifiedToOriginal(line) }
  // For unchanged lines: computes cumulative offset from insertions/deletions in prior hunks
  // For lines within a hunk: returns null (no mapping — line was changed)
}
```

This is used to map `lineSummary` entries (which use original-side line numbers) to modified-side line numbers so the "Before" dot can be placed on the correct modified-editor line.

### Step 1.3: Update `useLineReviewDecorations` for dual-dot rendering

**File:** `src/renderer/pages/review/editor/useLineReviewDecorations.js`

**New inputs:** `lineSummary`, `lineChanges` (or precomputed line mapping)

For each line in the modified editor, compute **two states**:

**Before state** (left dot):
- Use `modifiedToOriginal(line)` to find the corresponding original line
- If that original line is covered by a `lineSummary` entry → use that state
- If that original line exists but has no `lineSummary` entry → green (assumed approved)
- If the line is new (added in diff, no original counterpart) → no before dot / empty

**New state** (right dot):
1. **Explicitly reviewed in this session** → show that state's color
2. **In diff hunk, not yet reviewed** → grey/empty (`unreviewed-required`)
3. **Not in diff hunk, not explicitly reviewed** → carry forward the Before state (prior state persists until user reviews)

**Decoration approach:** Apply a single `glyphMarginClassName` per line that encodes both states:
```
line-review-dual before-{green|yellow|red|none} new-{green|yellow|red|grey}
```
CSS will render the two dots via pseudo-elements (see Step 1.5).

### Step 1.4: Pass `lineSummary` and `lineChanges` to the hook

**File:** `src/renderer/pages/review/editor/DiffEditor.jsx`

- Pass `lineSummary` from the open file data to the hook.
- Pass the `lineChanges` array (captured from `diffEditor.getLineChanges()` / `onDidUpdateDiff`) to the hook so it can build the line mapping internally.
- No need to expose the original editor ref — everything renders on the modified editor.

### Step 1.5: CSS for dual-dot rendering

**File:** `src/renderer/styles.css`

Replace single-dot glyph styles with a dual-dot approach. The `glyphMarginClassName` container uses `::before` (left/before dot) and `::after` (right/new dot):

```css
/* Base container for dual dots */
.line-review-dual {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
  width: 100%;
  height: 100%;
}

.line-review-dual::before,
.line-review-dual::after {
  content: '';
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

/* Before dot colors (::before) */
.before-green::before { background-color: #4caf50; }
.before-yellow::before { background-color: #ffc107; }
.before-red::before { background-color: #f44336; }
.before-none::before { background-color: transparent; border-color: transparent; }

/* New dot colors (::after) */
.new-green::after { background-color: #4caf50; }
.new-yellow::after { background-color: #ffc107; }
.new-red::after { background-color: #f44336; }
.new-grey::after { background-color: #666666; }
```

- The before dot should be slightly smaller or lower opacity to visually subordinate it.
- Selection highlight (`.line-review-dot-selected`) applies a blue outline around the whole container.

<!-- TODO: Monaco's glyph margin renders each className as an absolutely-positioned div.
     Need to verify that flexbox/pseudo-elements actually work inside Monaco's glyph margin
     containers. If not, alternative approaches:
     1. Use two separate decoration layers (glyphMarginClassName + marginClassName)
     2. Use a background-image approach with two colored circles as a data URI or gradient
     3. Use a custom ViewZone or overlay widget
     Prototype this early in implementation. -->

### Step 1.6: Handle inline diff mode

<!-- TODO: In inline diff mode (not side-by-side), the modified editor shows both old and new
     lines interleaved. The dual-dot approach still works (both dots on the modified editor),
     but the "Before" dot mapping may behave differently since deleted lines appear inline.
     Options:
     1. Force side-by-side mode when dual dots are active
     2. Adjust mapping logic for inline mode
     Recommend option 1 for initial implementation. -->

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
| `src/renderer/utils/lineMapping.js` | **Create** | 1 |
| `src/renderer/styles.css` | Modify | 1 & 2 |
| `src/renderer/pages/review/components/ReviewPopup.jsx` | Modify | 2 |
| `src/renderer/pages/review/components/IssueConflictModal.jsx` | **Create** | 2 |

---

## Open Questions (TODOs)

1. **LineSummary line number reference frame**: Do the `start`/`end` values in `lineSummary` correspond to the file at the HEAD event's commit? (Assumed yes — this would mean they map to the original-side editor content for a new commit review.)

2. **"Earliest commit in range" clarification**: When reviewing bulk commits, does "before" mean the state from before the entire range, or from the first commit in the range?

3. **Monaco glyph margin CSS feasibility**: Can Monaco's `glyphMarginClassName` divs support flexbox + pseudo-elements for dual dots? Needs early prototyping — fallback approaches listed in Step 1.5 TODO.

4. **Partial range splitting**: When a user approves a range but chooses "Maintain" for some issues within it, we need range-splitting logic. Confirm this is the desired behavior vs. simpler alternatives (e.g., all-or-nothing per range).
