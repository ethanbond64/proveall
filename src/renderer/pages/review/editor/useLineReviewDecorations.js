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
  const [popupState, setPopupState] = useState(null);
  const clickDisposableRef = useRef(null);
  const editorRef = useRef(null);

  // Store editor reference to ensure stability
  useEffect(() => {
    editorRef.current = editor;
  }, [editor]);

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
      // Generate decorations from line reviews
      const decorations = [];

      // Create a map to track which lines have been reviewed
      const reviewedLines = new Map();

      // First, collect all reviewed line ranges
      if (lineReviews && lineReviews.lineRanges && lineReviews.lineRanges.length > 0) {
        lineReviews.lineRanges.forEach(range => {
          for (let line = range.start; line <= range.end; line++) {
            reviewedLines.set(line, range.state || 'red');
          }
        });
      }

      // Now process all change blocks
      if (changeBlocks && changeBlocks.length > 0) {
        changeBlocks.forEach(block => {
          for (let line = block.startLine; line <= block.endLine; line++) {
            // Check if this line has a specific review
            const reviewState = reviewedLines.get(line);

            if (reviewState) {
              // Line has been reviewed - use that state
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
              // No specific review, but file has a default state
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
            } else {
              // Line hasn't been reviewed
              // In read-only mode (branch/issue), default to green
              // In interactive mode (commit), show as unreviewed
              const className = isInteractive ? 'line-review-dot-unreviewed' : 'line-review-dot-green';
              const hoverMessage = isInteractive ? 'Click to review' : 'Not explicitly reviewed (assumed good)';

              decorations.push({
                range: new monaco.Range(line, 1, line, 1),
                options: {
                  isWholeLine: false,
                  glyphMarginClassName: className,
                  glyphMarginHoverMessage: {
                    value: hoverMessage
                  }
                }
              });
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
  }, [lineReviews, changeBlocks, isInteractive]); // Note: not including editor in deps to avoid re-runs

  // Handle click events separately to avoid re-attaching on every update
  useEffect(() => {
    // Use stable editor reference
    const currentEditor = editorRef.current;
    if (!currentEditor || !isInteractive) {
      return;
    }

    // Clean up previous click handler if it exists
    if (clickDisposableRef.current) {
      clickDisposableRef.current.dispose();
      clickDisposableRef.current = null;
    }

    // Add click handler
    clickDisposableRef.current = currentEditor.onMouseDown(e => {
      if (e.target.type === monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN) {
        const lineNumber = e.target.position?.lineNumber;
        if (!lineNumber) return;

        // Find which block this line belongs to
        const block = changeBlocks.find(
          b => lineNumber >= b.startLine && lineNumber <= b.endLine
        );

        if (block) {
          // Find existing review for this range
          const existingReview = lineReviews?.lineRanges?.find(
            r => r.start === block.startLine && r.end === block.endLine
          );

          // Set popup state with position and current review info
          setPopupState({
            position: { x: e.event.posx, y: e.event.posy },
            range: { start: block.startLine, end: block.endLine },
            currentState: existingReview?.state || null,
            issueRef: existingReview?.issueRef || null,
            path
          });
        }
      }
    });

    return () => {
      if (clickDisposableRef.current) {
        clickDisposableRef.current.dispose();
        clickDisposableRef.current = null;
      }
    };
  }, [isInteractive, changeBlocks, lineReviews, path]);

  // Clean up decorations only when component unmounts
  useEffect(() => {
    return () => {
      // Use the stable editor reference for cleanup
      const currentEditor = editorRef.current;
      if (currentEditor && decorationsRef.current.length > 0) {
        try {
          currentEditor.deltaDecorations(decorationsRef.current, []);
          decorationsRef.current = [];
        } catch (error) {
          // Editor might be disposed, which is fine during cleanup
          decorationsRef.current = [];
        }
      }
    };
  }, []); // Empty deps - only run cleanup on unmount

  return { popupState, setPopupState };
}