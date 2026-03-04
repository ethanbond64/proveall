import { useEffect, useRef, useState } from 'react';
import * as monaco from 'monaco-editor';


/**
 * Minimal hook to add line review decorations to a Monaco editor
 */
export function useLineReviewDecorations(
  editor,
  changeBlocks,
  lineReviews,
  isInteractive,
  path
) {
  const decorationsRef = useRef([]);
  const dragHighlightRef = useRef([]);
  const [popupState, setPopupState] = useState(null);
  const clickDisposableRef = useRef(null);
  const editorRef = useRef(null);
  const dragStateRef = useRef({ isDragging: false, startLine: null });
  const lastClickedLineRef = useRef(null);
  const lineReviewsRef = useRef(lineReviews);

  // Store editor reference to ensure stability
  useEffect(() => {
    editorRef.current = editor;
  }, [editor]);

  // Keep lineReviews ref in sync for use in event handlers
  useEffect(() => {
    lineReviewsRef.current = lineReviews;
  }, [lineReviews]);

  // Update decorations when lineReviews or changeBlocks change
  useEffect(() => {
    // Use the stable editor reference
    const currentEditor = editorRef.current;
    if (!currentEditor) return;

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

      // Build a map of explicitly reviewed lines
      const reviewedLines = new Map();
      if (lineReviews && lineReviews.lineRanges && lineReviews.lineRanges.length > 0) {
        lineReviews.lineRanges.forEach(range => {
          for (let line = range.start; line <= range.end; line++) {
            reviewedLines.set(line, range.state || 'red');
          }
        });
      }

      // Iterate over ALL lines in the file
      for (let line = 1; line <= lineCount; line++) {
        const reviewState = reviewedLines.get(line);
        const isDiffLine = diffLines.has(line);

        if (reviewState) {
          // Line has an explicit review — show colored dot
          decorations.push({
            range: new monaco.Range(line, 1, line, 1),
            options: {
              isWholeLine: false,
              glyphMarginClassName: `line-review-dot-${reviewState}`,
              glyphMarginHoverMessage: {
                value: `Review status: ${reviewState}${isInteractive ? ' (click to change)' : ''}`
              }
            }
          });
        } else if (lineReviews && lineReviews.defaultState) {
          // No explicit review, but file has a default state
          decorations.push({
            range: new monaco.Range(line, 1, line, 1),
            options: {
              isWholeLine: false,
              glyphMarginClassName: `line-review-dot-${lineReviews.defaultState}`,
              glyphMarginHoverMessage: {
                value: `File default: ${lineReviews.defaultState}${isInteractive ? ' (click to change)' : ''}`
              }
            }
          });
        } else if (isDiffLine) {
          // Unreviewed diff line — required review
          if (isInteractive) {
            decorations.push({
              range: new monaco.Range(line, 1, line, 1),
              options: {
                isWholeLine: false,
                glyphMarginClassName: 'line-review-dot-unreviewed-required',
                glyphMarginHoverMessage: {
                  value: 'Click to review (required — part of diff)'
                }
              }
            });
          } else {
            decorations.push({
              range: new monaco.Range(line, 1, line, 1),
              options: {
                isWholeLine: false,
                glyphMarginClassName: 'line-review-dot-green',
                glyphMarginHoverMessage: {
                  value: 'Not explicitly reviewed (assumed good)'
                }
              }
            });
          }
        } else if (isInteractive) {
          // Non-diff line, no review — optional, plain gray dot
          decorations.push({
            range: new monaco.Range(line, 1, line, 1),
            options: {
              isWholeLine: false,
              glyphMarginClassName: 'line-review-dot-unreviewed',
              glyphMarginHoverMessage: {
                value: 'Click to review (optional)'
              }
            }
          });
        }
        // In read-only mode, non-diff lines with no review get no decoration
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
  }, [lineReviews, changeBlocks, isInteractive]); // Note: not including editor in deps to avoid re-runs

  // Helper to apply or clear drag highlight decorations
  const applyDragHighlight = (currentEditor, start, end) => {
    if (!currentEditor) return;
    const min = Math.min(start, end);
    const max = Math.max(start, end);
    const decorations = [];
    for (let line = min; line <= max; line++) {
      decorations.push({
        range: new monaco.Range(line, 1, line, 1),
        options: {
          isWholeLine: true,
          className: 'line-review-drag-highlight',
        }
      });
    }
    try {
      dragHighlightRef.current = currentEditor.deltaDecorations(
        dragHighlightRef.current || [],
        decorations
      );
    } catch (e) {
      dragHighlightRef.current = [];
    }
  };

  const clearDragHighlight = (currentEditor) => {
    if (!currentEditor) return;
    try {
      dragHighlightRef.current = currentEditor.deltaDecorations(
        dragHighlightRef.current || [],
        []
      );
    } catch (e) {
      dragHighlightRef.current = [];
    }
  };

  // Helper to find existing review for a given range
  const findExistingReview = (start, end) => {
    const reviews = lineReviewsRef.current;
    return reviews?.lineRanges?.find(
      r => r.start === start && r.end === end
    ) || null;
  };

  // Helper to open the stoplight popup for a range
  const openPopup = (position, start, end) => {
    const min = Math.min(start, end);
    const max = Math.max(start, end);
    const existingReview = findExistingReview(min, max);
    setPopupState({
      position,
      range: { start: min, end: max },
      currentState: existingReview?.state || null,
      issueRef: existingReview?.issueRef || null,
      path
    });
  };

  // Handle click, drag, and shift-click events
  useEffect(() => {
    const currentEditor = editorRef.current;
    if (!currentEditor || !isInteractive) {
      return;
    }

    // Clean up previous handlers
    if (clickDisposableRef.current) {
      clickDisposableRef.current.dispose();
      clickDisposableRef.current = null;
    }

    const disposables = [];

    // onMouseDown — start drag or handle shift-click
    disposables.push(currentEditor.onMouseDown(e => {
      if (e.target.type !== monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN) return;
      const lineNumber = e.target.position?.lineNumber;
      if (!lineNumber) return;

      // Shift-click: extend from last clicked line
      if (e.event.shiftKey && lastClickedLineRef.current != null) {
        const start = lastClickedLineRef.current;
        clearDragHighlight(currentEditor);
        openPopup({ x: e.event.posx, y: e.event.posy }, start, lineNumber);
        // Don't update lastClickedLine — keep anchor for further shift-clicks
        return;
      }

      // Normal click: start drag
      dragStateRef.current = { isDragging: true, startLine: lineNumber };
      lastClickedLineRef.current = lineNumber;
      applyDragHighlight(currentEditor, lineNumber, lineNumber);
    }));

    // onMouseMove — extend drag highlight
    disposables.push(currentEditor.onMouseMove(e => {
      if (!dragStateRef.current.isDragging) return;
      const lineNumber = e.target.position?.lineNumber;
      if (!lineNumber) return;
      applyDragHighlight(currentEditor, dragStateRef.current.startLine, lineNumber);
    }));

    // onMouseUp — finalize drag and open popup
    disposables.push(currentEditor.onMouseUp(e => {
      if (!dragStateRef.current.isDragging) return;
      const startLine = dragStateRef.current.startLine;
      dragStateRef.current = { isDragging: false, startLine: null };

      const lineNumber = e.target.position?.lineNumber || startLine;
      clearDragHighlight(currentEditor);
      openPopup({ x: e.event.posx, y: e.event.posy }, startLine, lineNumber);
    }));

    // Global mouseup — cancel drag if mouse released outside editor
    const handleGlobalMouseUp = () => {
      if (dragStateRef.current.isDragging) {
        dragStateRef.current = { isDragging: false, startLine: null };
        clearDragHighlight(currentEditor);
      }
    };
    document.addEventListener('mouseup', handleGlobalMouseUp);

    // Escape key — cancel drag
    const handleKeyDown = (e) => {
      if (e.key === 'Escape') {
        if (dragStateRef.current.isDragging) {
          dragStateRef.current = { isDragging: false, startLine: null };
          clearDragHighlight(currentEditor);
        }
      }
    };
    document.addEventListener('keydown', handleKeyDown);

    // Store a single disposable that cleans up everything
    clickDisposableRef.current = {
      dispose: () => {
        disposables.forEach(d => d.dispose());
        document.removeEventListener('mouseup', handleGlobalMouseUp);
        document.removeEventListener('keydown', handleKeyDown);
      }
    };

    return () => {
      if (clickDisposableRef.current) {
        clickDisposableRef.current.dispose();
        clickDisposableRef.current = null;
      }
    };
  }, [isInteractive, path]);

  // Clean up decorations only when component unmounts
  useEffect(() => {
    return () => {
      const currentEditor = editorRef.current;
      if (currentEditor) {
        try {
          if (decorationsRef.current.length > 0) {
            currentEditor.deltaDecorations(decorationsRef.current, []);
          }
          if (dragHighlightRef.current.length > 0) {
            currentEditor.deltaDecorations(dragHighlightRef.current, []);
          }
        } catch (error) {
          // Editor might be disposed, which is fine during cleanup
        }
        decorationsRef.current = [];
        dragHighlightRef.current = [];
      }
    };
  }, []); // Empty deps - only run cleanup on unmount

  // Wrap setPopupState to clear the shift-click anchor when popup is dismissed
  const dismissPopup = (value) => {
    if (value == null) {
      lastClickedLineRef.current = null;
    }
    setPopupState(value);
  };

  return { popupState, setPopupState: dismissPopup };
}