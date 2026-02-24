import React, { useEffect, useRef } from 'react';
import * as monaco from 'monaco-editor';
import '../../../styles.css';

//
// Map file extensions to Monaco language identifiers
//
function getLanguageFromFilename(filename) {
  const ext = filename.split('.').pop().toLowerCase();
  const languageMap = {
    js: 'javascript',
    jsx: 'javascript',
    ts: 'typescript',
    tsx: 'typescript',
    json: 'json',
    html: 'html',
    htm: 'html',
    css: 'css',
    scss: 'scss',
    less: 'less',
    md: 'markdown',
    markdown: 'markdown',
    py: 'python',
    rb: 'ruby',
    java: 'java',
    c: 'c',
    cpp: 'cpp',
    h: 'cpp',
    hpp: 'cpp',
    cs: 'csharp',
    go: 'go',
    rs: 'rust',
    php: 'php',
    swift: 'swift',
    kt: 'kotlin',
    scala: 'scala',
    sql: 'sql',
    sh: 'shell',
    bash: 'shell',
    zsh: 'shell',
    yaml: 'yaml',
    yml: 'yaml',
    xml: 'xml',
    svg: 'xml',
    txt: 'plaintext'
  };
  return languageMap[ext] || 'plaintext';
}

function CodeEditor({ filename, content}) {
  const editorRef = useRef(null);
  const containerRef = useRef(null);

  //
  // Initialize Monaco Editor once when component mounts
  //
  useEffect(() => {
    if (containerRef.current && !editorRef.current) {
      editorRef.current = monaco.editor.create(containerRef.current, {
        value: '',
        language: 'plaintext',
        theme: 'vs-dark',
        readOnly: true,  // Make editor read-only
        automaticLayout: true,
        minimap: { enabled: true },
        scrollBeyondLastLine: false,
        fontSize: 14,
        lineNumbers: 'on',
        renderLineHighlight: 'line',
        wordWrap: 'on'
      });
    }

    return () => {
      if (editorRef.current) {
        editorRef.current.dispose();
        editorRef.current = null;
      }
    };
  }, []);

  //
  // Update editor content when file changes
  //
  useEffect(() => {
    if (editorRef.current && filename) {
      const language = getLanguageFromFilename(filename);
      const model = editorRef.current.getModel();
      monaco.editor.setModelLanguage(model, language);
      editorRef.current.setValue(content);
    }
  }, [content, filename]);


  return <div className="editor-container" ref={containerRef}></div>;
}

export default CodeEditor;
