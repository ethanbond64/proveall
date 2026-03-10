import React, { useEffect, useRef, useState } from 'react';
import * as monaco from 'monaco-editor';
import { useLineReviewDecorations } from './useLineReviewDecorations';
import { useReviewContext } from '../ReviewContext';
import ReviewPopup from '../components/ReviewPopup';

// Helper function to get language from filename
function getLanguageFromFilename(filename) {
  const ext = filename.split('.').pop().toLowerCase();
  const languageMap = {
    js: 'javascript',
    jsx: 'javascript',
    ts: 'typescript',
    tsx: 'typescript',
    json: 'json',
    html: 'html',
    css: 'css',
    md: 'markdown',
    py: 'python',
    java: 'java',
    go: 'go',
    rs: 'rust',
    php: 'php',
    sql: 'sql',
    sh: 'shell',
    yaml: 'yaml',
    yml: 'yaml',
    xml: 'xml',
  };
  return languageMap[ext] || 'plaintext';
}

function DiffEditor({
  originalContent,
  modifiedContent,
  filename,
  path,
  lineReviews,
  lineSummary,
  readOnly = false
}) {
  const context = useReviewContext();
  const diffEditorRef = useRef(null);
  const containerRef = useRef(null);
  const modifiedEditorRef = useRef(null);
  const [renderSideBySide, setRenderSideBySide] = useState(true);
  const [changeBlocks, setChangeBlocks] = useState([]);
  const [lineChanges, setLineChanges] = useState(null);

  // Store setChangeBlocks action in a ref to avoid re-renders
  const setChangeBlocksRef = useRef(context.actions?.setChangeBlocks);
  useEffect(() => {
    setChangeBlocksRef.current = context.actions?.setChangeBlocks;
  }, [context.actions]);

  // Create and configure the Monaco diff editor (only once on mount)
  useEffect(() => {
    if (!containerRef.current) return;

    const diffEditor = monaco.editor.createDiffEditor(containerRef.current, {
      readOnly: true,
      renderSideBySide: renderSideBySide,
      originalEditable: false,
      automaticLayout: true,
      theme: 'vs-dark',
      minimap: { enabled: true },
      scrollBeyondLastLine: false,
      fontSize: 14,
      lineNumbers: 'on',
      renderWhitespace: 'selection',
      diffWordWrap: 'off',
      glyphMargin: true // Always show glyph margin for review decorations
    });

    diffEditorRef.current = diffEditor;

    return () => {
      if (diffEditorRef.current) {
        diffEditorRef.current.dispose();
      }
    };
  }, [readOnly]); // Remove renderSideBySide from dependencies

  // Update renderSideBySide option when it changes
  useEffect(() => {
    if (!diffEditorRef.current) return;

    diffEditorRef.current.updateOptions({
      renderSideBySide: renderSideBySide
    });
  }, [renderSideBySide]);

  // Update models when content changes
  useEffect(() => {
    if (!diffEditorRef.current) return;

    const language = getLanguageFromFilename(filename);

    // Create URIs
    const originalUri = monaco.Uri.parse(`file:///original/${filename}`);
    const modifiedUri = monaco.Uri.parse(`file:///modified/${filename}`);

    // Dispose existing models
    const existingOriginal = monaco.editor.getModel(originalUri);
    if (existingOriginal) existingOriginal.dispose();

    const existingModified = monaco.editor.getModel(modifiedUri);
    if (existingModified) existingModified.dispose();

    // Create new models
    const originalModel = monaco.editor.createModel(
      originalContent || '',
      language,
      originalUri
    );
    const modifiedModel = monaco.editor.createModel(
      modifiedContent || '',
      language,
      modifiedUri
    );

    // Set models
    diffEditorRef.current.setModel({
      original: originalModel,
      modified: modifiedModel
    });

    // Store modified editor reference
    modifiedEditorRef.current = diffEditorRef.current.getModifiedEditor();

    // Compute change blocks
    const computeChangeBlocks = () => {
      if (!diffEditorRef.current) return;

      const rawLineChanges = diffEditorRef.current.getLineChanges();
      const blocks = [];

      if (rawLineChanges) {
        rawLineChanges.forEach(change => {
          if (change.modifiedStartLineNumber && change.modifiedEndLineNumber) {
            blocks.push({
              startLine: change.modifiedStartLineNumber,
              endLine: change.modifiedEndLineNumber,
              originalStartLine: change.originalStartLineNumber,
              originalEndLine: change.originalEndLineNumber,
            });
          }
        });
      }

      setChangeBlocks(blocks);
      setLineChanges(rawLineChanges);

      // Report change blocks to context using ref
      if (setChangeBlocksRef.current) {
        setChangeBlocksRef.current(path, blocks);
      }
    };

    // Listen for diff computation
    const diffDisposable = diffEditorRef.current.onDidUpdateDiff(computeChangeBlocks);

    // Also compute initially after a small delay to ensure diff is ready
    setTimeout(computeChangeBlocks, 100);

    return () => {
      diffDisposable.dispose();
      originalModel.dispose();
      modifiedModel.dispose();
    };
  }, [originalContent, modifiedContent, filename, path]);

  // Add line review decorations (only when editor and change blocks are ready)
  const { popupState, setPopupState } = useLineReviewDecorations(
    modifiedEditorRef.current,
    changeBlocks,
    lineReviews,
    !readOnly, // isInteractive
    path,
    lineSummary,
    lineChanges
  );

  const toggleViewMode = () => {
    setRenderSideBySide(!renderSideBySide);
  };

  return (
    <div className="diff-editor-container">
      <div className="diff-editor-toolbar">
        <button onClick={toggleViewMode} className="diff-view-toggle-btn">
          {renderSideBySide ? 'Inline View' : 'Split View'}
        </button>
      </div>
      <div ref={containerRef} className="diff-editor-monaco-container" />

      {/* Render the line review popup when state is set */}
      {popupState && (
        <ReviewPopup
          mode="line"
          position={popupState.position}
          currentState={popupState.currentState}
          range={popupState.range}
          path={popupState.path}
          onClose={() => setPopupState(null)}
        />
      )}
    </div>
  );
}

export default DiffEditor;