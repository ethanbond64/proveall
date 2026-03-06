import React, { useState, useEffect, useRef } from 'react';
import '../styles.css';

function SettingsPage({ onBack }) {
  const [llmCommand, setLlmCommand] = useState('');
  const [llmArgs, setLlmArgs] = useState('');
  const [autoUpdate, setAutoUpdate] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState(null);
  const settingsRef = useRef(null);

  useEffect(() => {
    window.backendAPI.getSettings().then((settings) => {
      settingsRef.current = settings;
      setLlmCommand(settings.llm_command);
      setLlmArgs(settings.llm_args);
      setAutoUpdate(settings.auto_update);
    }).catch((err) => {
      console.error('Failed to load settings:', err);
      setStatus('Failed to load settings');
    });
  }, []);

  const handleSave = async () => {
    setSaving(true);
    setStatus(null);
    try {
      const updated = { ...settingsRef.current, llm_command: llmCommand, llm_args: llmArgs, auto_update: autoUpdate };
      await window.backendAPI.setSettings(updated);
      settingsRef.current = updated;
      setStatus('Settings saved');
    } catch (err) {
      console.error('Failed to save settings:', err);
      setStatus('Failed to save settings');
    } finally {
      setSaving(false);
    }
  };

  const handleRestoreDefaults = async () => {
    setStatus(null);
    try {
      const defaults = await window.backendAPI.resetSettings();
      settingsRef.current = defaults;
      setLlmCommand(defaults.llm_command);
      setLlmArgs(defaults.llm_args);
      setAutoUpdate(defaults.auto_update);
      setStatus('Defaults restored');
    } catch (err) {
      console.error('Failed to restore defaults:', err);
      setStatus('Failed to restore defaults');
    }
  };

  return (
    <div className="settings-page">
      <div className="settings-header">
        <button className="btn-secondary" onClick={onBack}>Back</button>
        <h2 className="settings-page-title">Settings</h2>
      </div>

      <div className="settings-section">
        <h3 className="settings-section-title">LLM Provider</h3>

        <div className="form-group">
          <label>Command</label>
          <input
            type="text"
            value={llmCommand}
            onChange={(e) => setLlmCommand(e.target.value)}
            placeholder="claude"
          />
        </div>

        <div className="form-group">
          <label>Arguments</label>
          <input
            type="text"
            value={llmArgs}
            onChange={(e) => setLlmArgs(e.target.value)}
            placeholder="--print --dangerously-skip-permissions ..."
          />
        </div>

        <div className="settings-actions">
          <button
            className="btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? 'Saving...' : 'Save'}
          </button>
          <button
            className="btn-secondary"
            onClick={handleRestoreDefaults}
          >
            Restore Defaults
          </button>
        </div>

        {status && <p className="settings-status">{status}</p>}
      </div>

      <div className="settings-section" style={{ marginTop: 16 }}>
        <h3 className="settings-section-title">Updates</h3>
        <label className="settings-checkbox-label">
          <input
            type="checkbox"
            checked={autoUpdate}
            onChange={(e) => setAutoUpdate(e.target.checked)}
          />
          Automatically install updates
        </label>
      </div>
    </div>
  );
}

export default SettingsPage;
