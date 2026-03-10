import { useEffect, useRef, useState, useMemo } from 'react';
import * as monaco from 'monaco-editor';
import { buildLineMapping } from '../../../utils/lineMapping.js';


/**
 * Hook to add dual-column line review decorations to a Monaco editor.
 * Renders two dots per line in the glyph margin:
 *   - Left dot ("Before"): prior review state from lineSummary
 *   - Right dot ("New"): current session review state
 */
export function useLineReviewDecorations(
  editor,
  changeBlocks,
  lineReviews,
  isInteractive,
  path,
  lineSummary,
  lineChanges
) {
  const decorationsRef = useRef([]);
  const selectionHighlightRef = useRef([]);
  const [popupState, setPopupState] = useState(null);
  const clickDisposableRef = useRef(null);
  const selectionDisposableRef = useRef(null);
  const editorRef = useRef(null);
  const lineReviewsRef = useRef(lineReviews);
  const changeBlocksRef = useRef(changeBlocks);

  // Store editor reference to ensure stability
  useEffect(() => {
    editorRef.current = editor;
  }, [editor]);

  // Keep refs in sync for use in event handlers
  useEffect(() => {
    lineReviewsRef.current = lineReviews;
  }, [lineReviews]);

  useEffect(() => {
    changeBlocksRef.current = changeBlocks;
  }, [changeBlocks]);

  // Helper to apply blue outline highlights to a range of glyph dots
  const applyBlueHighlight = (currentEditor, startLine, endLine) => {
    const decorations = [];
    for (let line = startLine; line <= endLine; line++) {
      decorations.push({
        range: new monaco.Range(line, 1, line, 1),
        options: {
          isWholeLine: false,
          glyphMarginClassName: 'line-review-dot-selected',
        }
      });
    }
    try {
      selectionHighlightRef.current = currentEditor.deltaDecorations(
        selectionHighlightRef.current || [],
        decorations
      );
    } catch (e) {
      selectionHighlightRef.current = [];
    }
  };

  const clearBlueHighlight = (currentEditor) => {
    try {
      selectionHighlightRef.current = currentEditor.deltaDecorations(
        selectionHighlightRef.current || [],
        []
      );
    } catch (e) {
      selectionHighlightRef.current = [];
    }
  };

  // Build line mapping from lineChanges (memoized)
  const mapping = useMemo(() => buildLineMapping(lineChanges), [lineChanges]);

  // Update decorations when lineReviews, changeBlocks, lineSummary, or mapping change
  useEffect(() => {
    // Use the stable editor reference
    const currentEditor = editorRef.current;
    if (!currentEditor) return;

    // Clear blue selection highlight when reviews update (color was applied)
    clearBlueHighlight(currentEditor);

    // Wait for the editor model to be ready
    const model = currentEditor.getModel();
    if (!model) return;

    // Function to apply decorations
    const applyDecorations = () => {
      const decorations = [];
      const lineCount = model.getLineCount();

      // Build a set of lines that are within diff change blocks (required review)
      const diffLines = new Set();
      if (changeBlocks && changeBlocks.length > 0) {
        changeBlocks.forEach(block => {
          for (let line = block.startLine; line <= block.endLine; line++) {
            diffLines.add(line);
          }
        });
      }

      // Build a map of explicitly reviewed lines (new session reviews, modified-side line numbers)
      const reviewedLines = new Map();
      if (lineReviews && lineReviews.lineRanges && lineReviews.lineRanges.length > 0) {
        lineReviews.lineRanges.forEach(range => {
          for (let line = range.start; line <= range.end; line++) {
            reviewedLines.set(line, range.state || 'red');
          }
        });
      }

      // Build a map of prior review states from lineSummary (original-side line numbers)
      // lineSummary entries: { start, end, state, issueRef }
      const priorReviewOriginal = new Map();
      if (lineSummary && lineSummary.length > 0) {
        lineSummary.forEach(entry => {
          for (let line = entry.start; line <= entry.end; line++) {
            priorReviewOriginal.set(line, entry.state);
          }
        });
      }

      // Helper: get the "before" state for a modified-side line
      const getBeforeState = (modLine) => {
        const origLine = mapping.modifiedToOriginal(modLine);
        if (origLine === null) {
          // Line is inside a diff hunk (new/changed line) — no prior counterpart
          return 'none';
        }
        // Check if this original line had a prior review
        const priorState = priorReviewOriginal.get(origLine);
        // Lines with no prior review entry are assumed approved (green)
        return priorState || 'green';
      };

      // Helper: get the "new" state for a modified-side line
      const getNewState = (modLine) => {
        // 1. Explicitly reviewed in this session
        const reviewState = reviewedLines.get(modLine);
        if (reviewState) return reviewState;

        // 2. File-level default state applies to all lines
        if (lineReviews && lineReviews.defaultState) {
          return lineReviews.defaultState;
        }

        // 3. In diff hunk, not yet reviewed → grey (unreviewed-required)
        if (diffLines.has(modLine)) return 'grey';

        // 4. Not in diff hunk → carry forward the before state
        return getBeforeState(modLine);
      };

      // Iterate over ALL lines in the modified editor
      for (let line = 1; line <= lineCount; line++) {
        const beforeState = getBeforeState(line);
        const newState = getNewState(line);

        // In read-only mode, skip lines with no meaningful state
        if (!isInteractive && beforeState === 'none' && newState === 'grey') {
          continue;
        }

        const className = `line-review-dual before-${beforeState} new-${newState}`;

        const hoverParts = [];
        if (beforeState !== 'none') {
          hoverParts.push(`Before: ${beforeState}`);
        }
        hoverParts.push(`New: ${newState === 'grey' ? 'unreviewed' : newState}`);
        if (isInteractive) {
          hoverParts.push('(click to change)');
        }

        decorations.push({
          range: new monaco.Range(line, 1, line, 1),
          options: {
            isWholeLine: false,
            glyphMarginClassName: className,
            glyphMarginHoverMessage: {
              value: hoverParts.join(' | ')
            }
          }
        });
      }

      // Clear existing decorations and apply new ones in a single operation
      try {
        const oldDecorations = decorationsRef.current || [];
        decorationsRef.current = currentEditor.deltaDecorations(oldDecorations, decorations);
      } catch (error) {
        console.error('Failed to apply decorations:', error);
        // Try to recover by clearing and reapplying
        decorationsRef.current = [];
        try {
          if (currentEditor) {
            decorationsRef.current = currentEditor.deltaDecorations([], decorations);
          }
        } catch (retryError) {
          console.error('Failed to recover decorations:', retryError);
        }
      }
    };

    // Apply decorations immediately
    applyDecorations();

    // Also apply after a short delay to handle any async updates
    const timeoutId = setTimeout(applyDecorations, 50);

    return () => {
      clearTimeout(timeoutId);
    };
  }, [lineReviews, changeBlocks, lineSummary, mapping, isInteractive]); // Note: not including editor in deps to avoid re-runs

  // Highlight glyph dots with blue outline when lines are selected in the editor
  useEffect(() => {
    const currentEditor = editorRef.current;
    if (!currentEditor || !isInteractive) return;

    if (selectionDisposableRef.current) {
      selectionDisposableRef.current.dispose();
      selectionDisposableRef.current = null;
    }

    const updateSelectionHighlight = () => {
      const selection = currentEditor.getSelection();

      if (selection && !selection.isEmpty()) {
        const startLine = selection.startLineNumber;
        let endLine = selection.endLineNumber;
        if (selection.endColumn === 1 && endLine > startLine) {
          endLine = endLine - 1;
        }
        applyBlueHighlight(currentEditor, startLine, endLine);
      } else {
        clearBlueHighlight(currentEditor);
      }
    };

    selectionDisposableRef.current = currentEditor.onDidChangeCursorSelection(updateSelectionHighlight);

    return () => {
      if (selectionDisposableRef.current) {
        selectionDisposableRef.current.dispose();
        selectionDisposableRef.current = null;
      }
      try {
        if (currentEditor && selectionHighlightRef.current.length > 0) {
          currentEditor.deltaDecorations(selectionHighlightRef.current, []);
          selectionHighlightRef.current = [];
        }
      } catch (e) {
        selectionHighlightRef.current = [];
      }
    };
  }, [editor, isInteractive]);

  // Handle glyph margin clicks
  useEffect(() => {
    const currentEditor = editorRef.current;
    if (!currentEditor || !isInteractive) {
      return;
    }

    // Clean up previous handler
    if (clickDisposableRef.current) {
      clickDisposableRef.current.dispose();
      clickDisposableRef.current = null;
    }

    const disposables = [];

    disposables.push(currentEditor.onMouseDown(e => {
      if (e.target.type !== monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN) return;
      const lineNumber = e.target.position?.lineNumber;
      if (!lineNumber) return;

      const selection = currentEditor.getSelection();
      let start = lineNumber;
      let end = lineNumber;

      if (selection && !selection.isEmpty()) {
        const selStart = selection.startLineNumber;
        const selEnd = selection.endLineNumber;
        // If the clicked line falls within the selection, use the selection range
        if (lineNumber >= selStart && lineNumber <= selEnd) {
          start = selStart;
          // If selection ends at column 1 of a line, the user didn't actually
          // select content on that line — exclude it
          end = (selection.endColumn === 1 && selEnd > selStart) ? selEnd - 1 : selEnd;
        }
      } else {
        // No selection: if the line is inside a hunk, use the full hunk range
        const blocks = changeBlocksRef.current || [];
        const block = blocks.find(b => lineNumber >= b.startLine && lineNumber <= b.endLine);
        if (block) {
          start = block.startLine;
          end = block.endLine;
          applyBlueHighlight(currentEditor, start, end);
        }
      }

      // Look up existing review for this exact range
      const reviews = lineReviewsRef.current;
      const existingReview = reviews?.lineRanges?.find(
        r => r.start === start && r.end === end
      ) || null;

      setPopupState({
        position: { x: e.event.posx, y: e.event.posy },
        range: { start, end },
        currentState: existingReview?.state || null,
        issueRef: existingReview?.issueRef || null,
        path
      });
    }));

    clickDisposableRef.current = {
      dispose: () => disposables.forEach(d => d.dispose())
    };

    return () => {
      if (clickDisposableRef.current) {
        clickDisposableRef.current.dispose();
        clickDisposableRef.current = null;
      }
    };
  }, [editor, isInteractive, path]);

  // Clean up decorations only when component unmounts
  useEffect(() => {
    return () => {
      const currentEditor = editorRef.current;
      if (currentEditor) {
        try {
          if (decorationsRef.current.length > 0) {
            currentEditor.deltaDecorations(decorationsRef.current, []);
          }
          if (selectionHighlightRef.current.length > 0) {
            currentEditor.deltaDecorations(selectionHighlightRef.current, []);
          }
        } catch (error) {
          // Editor might be disposed, which is fine during cleanup
        }
        decorationsRef.current = [];
        selectionHighlightRef.current = [];
      }
    };
  }, []); // Empty deps - only run cleanup on unmount

  return { popupState, setPopupState };
}