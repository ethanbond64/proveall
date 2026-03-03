import React, { useState, useEffect } from 'react';
import '../styles.css';

function SettingsPage({ onBack }) {
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState(null);

  useEffect(() => {
    window.backendAPI.getLlmSettings().then((settings) => {
      setCommand(settings.command);
      setArgs(settings.args);
    }).catch((err) => {
      console.error('Failed to load settings:', err);
      setStatus('Failed to load settings');
    });
  }, []);

  const handleSave = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await window.backendAPI.updateLlmSettings(command, args);
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
      const defaults = await window.backendAPI.resetLlmSettings();
      setCommand(defaults.command);
      setArgs(defaults.args);
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
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="claude"
          />
        </div>

        <div className="form-group">
          <label>Arguments</label>
          <input
            type="text"
            value={args}
            onChange={(e) => setArgs(e.target.value)}
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
    </div>
  );
}

export default SettingsPage;
