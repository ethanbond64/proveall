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