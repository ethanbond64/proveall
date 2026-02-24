import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './styles.css';
import './tauriAPI'; // Setup Tauri API

//
// Standard react entrypoint script.
//
const root = createRoot(document.getElementById('root'));
root.render(<App />);
