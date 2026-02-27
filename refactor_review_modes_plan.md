# Plan: Refactor Review Modes from 3 to 2

## Executive Summary
This plan outlines the refactoring of the review system to simplify from three modes (`commit`, `branch`, `issue`) to two modes (`commit`, `branch`). The `issue` mode functionality will be merged into `branch` mode as conditional behavior based on the presence of an `issueId` parameter.

## Current State
- **Three review modes**: `commit`, `branch`, `issue`
- **Affected methods**:
  - `get_review_file_data`
  - `get_review_file_system_data`
- **Issue mode**: Currently a separate mode that filters data by issue ID

## Target State
- **Two review modes**: `commit`, `branch`
- **Issue functionality**: Becomes conditional behavior within `branch` mode
- **Trigger**: When `mode === 'branch' && issueId` is present, apply issue-specific filtering

## Phase 1 Discoveries (Completed)

### Files with 'issue' mode references:
1. **Backend (Rust)**:
   - `tauri_src/src/services/review_service.rs` - Main implementation with ReviewType enum
   - `tauri_src/src/commands/review_commands.rs` - Tauri command interfaces
   - `tauri_src/src/repositories/mod.rs` - XRef query methods

2. **Frontend (JavaScript/React)**:
   - `src/renderer/App.jsx` - Sets mode to 'issue' when navigating (line 77)
   - `src/renderer/pages/project/ProjectPage.jsx` - Passes 'issue' mode (line 221)
   - `src/renderer/pages/review/ReviewContext.jsx` - Dynamically determines mode based on issueId
   - `src/renderer/pages/review/issuedata/IssueDataPanel.jsx` - UI logic based on mode
   - `src/renderer/pages/review/ReviewProjectPage.jsx` - UI logic based on mode
   - `src/renderer/tauriAPI.js` - API calls pass reviewType parameter

### Backend Implementation Analysis:

#### `get_review_file_system_data` method:
- Uses `ReviewType::parse` to convert string to enum (lines 46)
- Match statement on ReviewType (lines 52-69)
- Issue mode:
  - Calls `find_event_for_issue` to get issue's event
  - Uses `build_issue_touched_files` with `join_list_by_event_and_issue`
  - Returns single issue via `fetch_single_issue`

#### `get_review_file_data` method:
- Similar structure with `ReviewType::parse` (lines 195)
- Match statement on ReviewType (lines 207-248)
- Issue mode:
  - Calls `find_event_for_issue` to get issue's event
  - Gets diff from issue event's parent commit
  - Uses `build_issue_line_summary` with `join_list_by_event_and_issue`
  - Returns single issue via `fetch_single_issue`

### XRef Query Methods:
1. **`join_list_by_event`** - Filters by event_id and branch_context_id only
2. **`join_list_by_event_and_issue`** - Filters by event_id, issue_id, AND branch_context_id

### Key Functions to Refactor:
- `build_branch_touched_files` → needs conditional issueId filtering
- `build_branch_line_summary` → needs conditional issueId filtering
- `build_issues_from_event` → needs conditional issueId filtering
- `build_issue_touched_files` → can be merged into branch version
- `build_issue_line_summary` → can be merged into branch version

## Implementation Tasks

### Phase 1: Discovery & Analysis ✅ COMPLETED
1. **Search for all occurrences of 'issue' mode in the codebase**
   - Backend API methods
   - Frontend/client code
   - Type definitions
   - Documentation

2. **Analyze `get_review_file_data` method**
   - Understand current mode handling logic
   - Identify all conditional branches based on mode
   - Document xref table queries

3. **Analyze `get_review_file_system_data` method**
   - Understand current mode handling logic
   - Identify all conditional branches based on mode
   - Document xref table queries

4. **Identify all xref table queries**
   - Map out all queries that need conditional issueId filtering
   - Document the filtering logic needed for each

### Frontend Mode Logic Patterns:
In `ReviewContext.jsx`, the code dynamically determines the mode:
```javascript
// Line 382:
const reviewType = issueId ? 'issue' : (mode === 'branch' ? 'branch' : 'commit');

// Line 526:
const reviewType = state.issueId ? 'issue' : (state.mode === 'branch' ? 'branch' : 'commit');

// Line 616:
const eventType = state.mode === 'issue' ? 'resolution' : 'commit';
```

### Phase 2: Backend Refactoring

5. **Update `get_review_file_data` method**
   - Remove 'issue' as a valid mode option
   - For Branch mode:
     - Pass `issue_id` to `build_branch_line_summary`
     - Pass `issue_id` to `build_issues_from_event`
     - Remove separate Issue branch in match statement
   - Content and diff logic remains the same (based on commit)

6. **Update `get_review_file_system_data` method**
   - Remove 'issue' as a valid mode option
   - For Branch mode:
     - Pass `issue_id` to `build_branch_touched_files`
     - Pass `issue_id` to `build_issues_from_event`
     - Always return ALL files from git diff
     - File status determined by issue-filtered xref queries when issueId present

7. **Update mode validation logic**
   - Find all validation code that checks for valid modes
   - Update to only accept `['commit', 'branch']`
   - Remove 'issue' from any enum or validation arrays

### Phase 3: Frontend/Client Updates

8. **Update frontend/client code**
   - Find all places sending `mode: 'issue'`
   - Change to `mode: 'branch'` while ensuring `issueId` is included
   - Update any mode selection UI components
   - Update any mode-related state management

### Phase 4: Testing & Documentation

9. **Test commit mode functionality**
   - Ensure commit mode works as before
   - No regression in commit-specific features

10. **Test branch mode without issueId**
    - Standard branch mode behavior
    - No issue-specific filtering applied

11. **Test branch mode with issueId**
    - Former issue mode behavior
    - Verify issueId filtering is correctly applied to xref queries
    - Ensure feature parity with previous issue mode

12. **Update documentation**
    - API documentation
    - TypeScript type definitions
    - Remove `issue` from mode type unions
    - Update any developer documentation

## Technical Details

### Example: Updated `build_branch_touched_files` Logic
```rust
fn build_branch_touched_files(
    conn: &mut SqliteConnection,
    project_path: &str,
    event_id: Option<&str>,
    branch_context_id: &str,
    base_branch: &str,
    issue_id: Option<&str>,  // NEW PARAMETER
) -> Result<Vec<TouchedFile>, AppError> {
    // Always get ALL files from git diff
    let range = format!("{}..HEAD", base_branch);
    let diff_files = diff_changed_files(project_path, &[&range])?;

    // Get composites filtered by issue if provided
    let composite_paths: HashSet<String> = if let Some(eid) = event_id {
        let composites = if let Some(iid) = issue_id {
            // Filter by specific issue
            join_list_by_event_and_issue(conn, eid, iid, branch_context_id)?
        } else {
            // Get all composites for this event
            join_list_by_event(conn, eid, branch_context_id)?
        };

        composites.into_iter()
            .map(|(_, composite)| composite.relative_file_path)
            .collect()
    } else {
        HashSet::new()
    };

    // File is red if it has composites (filtered by issue if provided)
    Ok(diff_files.into_iter().map(|f| TouchedFile {
        name: extract_file_name(&f.path),
        path: f.path,
        diff_mode: Some(f.status),
        state: if composite_paths.contains(&f.path) { "red" } else { "green" }.to_string(),
    }).collect())
}
```

### Key Changes to XRef Queries
When `issueId` is present in branch mode, filter xref queries:
```sql
-- Without issueId (show files with ANY issues):
WHERE xref.event_id = $eventId AND xref.branch_context_id = $branchContextId

-- With issueId (show files with SPECIFIC issue):
WHERE xref.event_id = $eventId AND xref.issue_id = $issueId AND xref.branch_context_id = $branchContextId
```

### Type Definition Updates
Before:
```typescript
type ReviewMode = 'commit' | 'branch' | 'issue';
```

After:
```typescript
type ReviewMode = 'commit' | 'branch';
```

### API Signature Changes
Methods will maintain the same signatures but with updated behavior:
- `issueId` parameter remains optional
- When provided with `mode: 'branch'`, triggers issue-specific filtering
- `mode: 'issue'` will be rejected as invalid

## Risk Mitigation
- **Backward compatibility**: Consider adding temporary deprecation warning for 'issue' mode
- **Testing**: Comprehensive testing of all three scenarios (commit, branch, branch+issueId)
- **Rollback plan**: Keep original code in version control for quick revert if needed

## Refactoring Strategy

### Backend Strategy:

#### Key Change: Branch Mode Behavior
**Important**: When in branch mode (regardless of issueId presence):
- Always show ALL touched files from the git diff
- The file status (red/green) is determined by:
  - If `issueId` is None: Check if file has ANY composites in xref
  - If `issueId` is Some: Check if file has composites for THAT SPECIFIC issue

This ensures consistent UI - users always see all branch changes, but the status indicates which files have issues (filtered by specific issue if provided).

1. **Update Branch Functions with Conditional Filtering**:
   - `build_branch_touched_files`:
     - Always get all files from git diff
     - Add `issue_id: Option<&str>` parameter
     - Use conditional xref query to determine file status
   - `build_branch_line_summary`:
     - Add `issue_id: Option<&str>` parameter
     - Filter line summaries by issue if provided
   - `build_issues_from_event`:
     - Add `issue_id: Option<&str>` parameter
     - Return single issue if provided, all issues otherwise

2. **Conditional XRef Queries**:
   ```rust
   // For determining file status and line summaries:
   let composites = if let Some(issue_id) = issue_id {
       // Filter by both event_id AND issue_id
       join_list_by_event_and_issue(conn, event_id, issue_id, branch_context_id)?
   } else {
       // Filter by event_id only
       join_list_by_event(conn, event_id, branch_context_id)?
   };
   ```

3. **Mode Handling**:
   - Remove `ReviewType::Issue` enum variant
   - Remove separate `build_issue_*` functions
   - In Branch mode, apply conditional filtering based on `issue_id.is_some()`

### Frontend Strategy:
1. **Change mode setting**:
   - Replace `setReviewMode('issue')` with `setReviewMode('branch')`
   - Ensure issueId is passed in context

2. **Update mode checks**:
   - Replace `mode === 'issue'` with `mode === 'branch' && issueId`
   - Update ReviewContext to handle branch mode with issueId

## Success Criteria
- [ ] Only 2 modes accepted: 'commit' and 'branch'
- [ ] Issue filtering works correctly when issueId is provided with branch mode
- [ ] No regression in existing commit and branch functionality
- [ ] All tests pass
- [ ] Frontend correctly uses new mode pattern
- [ ] Documentation is updated

## Timeline Estimate
- Discovery & Analysis: 1-2 hours
- Backend Refactoring: 2-3 hours
- Frontend Updates: 1-2 hours
- Testing & Documentation: 1-2 hours
- **Total: 5-9 hours**