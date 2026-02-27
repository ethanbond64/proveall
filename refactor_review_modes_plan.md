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

## Implementation Tasks

### Phase 1: Discovery & Analysis
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

### Phase 2: Backend Refactoring

5. **Update `get_review_file_data` method**
   - Remove 'issue' as a valid mode option
   - Refactor mode checks from:
     ```typescript
     if (mode === 'issue') {
       // issue-specific logic with issueId filtering
     } else if (mode === 'branch') {
       // branch logic
     }
     ```
   - To:
     ```typescript
     if (mode === 'branch') {
       if (issueId) {
         // former issue mode logic - apply issueId filtering to xref queries
       } else {
         // standard branch logic
       }
     }
     ```
   - Ensure all xref table queries include issueId filtering when present

6. **Update `get_review_file_system_data` method**
   - Apply same refactoring pattern as `get_review_file_data`
   - Ensure consistency in conditional logic

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

### Key Changes to XRef Queries
When `mode === 'branch' && issueId` is present, add the following to xref table queries:
```sql
AND xref.issue_id = $issueId
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