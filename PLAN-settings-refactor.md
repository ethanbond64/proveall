# Plan: Settings API Refactor

## Overview

Replace the three LLM-specific settings commands (`get_llm_settings`, `update_llm_settings`, `reset_llm_settings`) with three generic settings commands (`get_settings`, `set_settings`, `reset_settings`) that operate on the full settings object. This makes it trivial to add new settings (e.g. `auto_update`) without new Rust commands or API wrappers.

## Current State

**Rust side:**
- `LlmConfig { command, args }` in `tauri_src/src/utils/llm.rs`
- `load_settings()` / `save_settings()` read/write `settings.json`
- `default_llm_config()` returns defaults
- `LlmState { config, app_data_dir }` managed as Tauri state in `lib.rs`
- Three commands in `settings_commands.rs`: `get_llm_settings`, `update_llm_settings`, `reset_llm_settings`
- `llm_service.rs` uses `LlmConfig` via `call_llm()`

**JS side:**
- `tauriAPI.js` exposes `getLlmSettings()`, `updateLlmSettings(command, args)`, `resetLlmSettings()`
- `SettingsPage.jsx` calls these to load/save LLM command and args

**Settings file:** `~/.local/share/com.proveall/settings.json` (macOS: `~/Library/Application Support/`)

## Target State

**New settings schema:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub llm_command: String,
    pub llm_args: String,
    pub auto_update: bool,
}
```

**Three generic commands:**
- `get_settings() -> AppSettings`
- `set_settings(settings: AppSettings) -> ()`
- `reset_settings() -> AppSettings`

**JS API:**
- `getSettings() -> settings object`
- `setSettings(settings) -> void`
- `resetSettings() -> settings object (defaults)`

The frontend sends/receives the full settings object every time. No field-specific commands.

## Implementation Steps

### Step 1: Create `tauri_src/src/utils/settings.rs`

New file with:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_llm_command")]
    pub llm_command: String,
    #[serde(default = "default_llm_args")]
    pub llm_args: String,
    #[serde(default)]
    pub auto_update: bool,
}
```

- `default_settings() -> AppSettings`
- `load_settings(app_data_dir) -> AppSettings`
- `save_settings(app_data_dir, &AppSettings) -> Result`
- Helper: `to_llm_config(&self) -> LlmConfig` (for backward compat with `call_llm`)

Use `#[serde(default)]` on all fields so existing `settings.json` files with only `command`/`args` deserialize gracefully. Handle the field rename from `command`→`llm_command` and `args`→`llm_args` with `#[serde(alias = "command")]` and `#[serde(alias = "args")]`.

### Step 2: Refactor `tauri_src/src/utils/llm.rs`

- Remove `LlmConfig` struct, `default_llm_config()`, `load_settings()`, `save_settings()`, `settings_path()`
- Keep only `call_llm()` — it still takes a simple config struct for command + args
- Create a minimal `LlmConfig { command, args }` that is only used by `call_llm()` (not serialized to disk)
- Or: keep `LlmConfig` as-is but only for the `call_llm` function signature; `AppSettings` has a `to_llm_config()` method

### Step 3: Update `tauri_src/src/utils/mod.rs`

Add `pub mod settings;`.

### Step 4: Refactor `tauri_src/src/lib.rs`

Replace `LlmState` with `SettingsState`:
```rust
pub struct SettingsState {
    pub settings: Arc<RwLock<AppSettings>>,
    pub app_data_dir: PathBuf,
}
```

Update startup code to use `settings::load_settings()` instead of `llm::load_settings()`.

Update invoke handler to register new commands: `get_settings`, `set_settings`, `reset_settings` (remove old three).

### Step 5: Rewrite `tauri_src/src/commands/settings_commands.rs`

Replace the three LLM-specific commands with:

```rust
#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Result<AppSettings, String> { ... }

#[tauri::command]
pub fn set_settings(state: State<'_, SettingsState>, settings: AppSettings) -> Result<(), String> { ... }

#[tauri::command]
pub fn reset_settings(state: State<'_, SettingsState>) -> Result<AppSettings, String> { ... }
```

### Step 6: Update `tauri_src/src/commands/llm_commands.rs`

This file accesses `LlmState` to get the LLM config for `fix_issue`. Update it to use `SettingsState` and call `.to_llm_config()`.

### Step 7: Update `src/renderer/tauriAPI.js`

Replace:
```js
getLlmSettings: () => invoke('get_llm_settings'),
updateLlmSettings: (command, args) => invoke('update_llm_settings', { command, args }),
resetLlmSettings: () => invoke('reset_llm_settings'),
```

With:
```js
getSettings: () => invoke('get_settings'),
setSettings: (settings) => invoke('set_settings', { settings }),
resetSettings: () => invoke('reset_settings'),
```

### Step 8: Update `src/renderer/pages/SettingsPage.jsx`

- Load full settings object on mount: `window.backendAPI.getSettings()`
- On save, send full object: `window.backendAPI.setSettings({ llm_command, llm_args, ...other fields })`
- On reset, receive full defaults: `window.backendAPI.resetSettings()`
- For now, the UI only shows/edits `llm_command` and `llm_args` (same as before). The `auto_update` field will be added by the updater plan.

## File Change Summary

| File | Action |
|------|--------|
| `tauri_src/src/utils/settings.rs` | **New** — AppSettings struct, load/save, defaults |
| `tauri_src/src/utils/mod.rs` | Add `pub mod settings` |
| `tauri_src/src/utils/llm.rs` | Remove settings code, keep only `call_llm` + `LlmConfig` |
| `tauri_src/src/lib.rs` | Replace `LlmState` with `SettingsState`, update commands |
| `tauri_src/src/commands/settings_commands.rs` | Rewrite with generic get/set/reset |
| `tauri_src/src/commands/llm_commands.rs` | Use `SettingsState` instead of `LlmState` |
| `src/renderer/tauriAPI.js` | Replace 3 LLM methods with 3 generic methods |
| `src/renderer/pages/SettingsPage.jsx` | Use new API methods |
