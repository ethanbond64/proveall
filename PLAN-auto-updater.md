# Plan: Tauri Auto-Updater for ProveAll

## Overview

Add auto-update functionality using `tauri-plugin-updater` so users are notified when a new version is available, with an optional setting to auto-install updates.

## Decisions

- **Update endpoint**: Static `latest.json` file hosted on GitHub Releases
- **Update logic**: JavaScript/frontend-driven via `@tauri-apps/plugin-updater` JS API
- **Key generation**: Manual (not covered by this plan)

## Current State

- **Version**: 0.4.1, tracked in 4 places (package.json, tauri.conf.json, Cargo.toml, CHANGELOG.md)
- **Releases**: Automated via GitHub Actions on push to `main`. Builds macOS aarch64 `.dmg` and `.app.tar.gz`, creates a GitHub Release with version tag.
- **Settings**: `settings.json` stores `LlmConfig { command, args }` — loaded/saved in `tauri_src/src/utils/llm.rs`. Settings commands in `settings_commands.rs`. UI in `SettingsPage.jsx`.
- **Frontend**: React 18, state-based routing in `App.jsx`. All Tauri commands called via `window.backendAPI` in `tauriAPI.js`.
- **GitHub repo**: `https://github.com/ethanbond64/proveall`

## Implementation Steps

### Step 1: Install plugin dependencies

**Cargo** — add to `tauri_src/Cargo.toml` dependencies:
```toml
tauri-plugin-updater = "2"
```

**npm** — add to `package.json`:
```
@tauri-apps/plugin-updater
```

### Step 2: Register the updater plugin in Rust

In `tauri_src/src/lib.rs`, add the plugin to the builder:
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_updater::init())
    .plugin(tauri_plugin_dialog::init())
    // ... rest unchanged
```

### Step 3: Configure updater in `tauri.conf.json`

Add the `plugins.updater` section:
```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/ethanbond64/proveall/releases/latest/download/latest.json"
      ],
      "pubkey": "PLACEHOLDER_UNTIL_KEY_IS_GENERATED"
    }
  }
}
```

The `pubkey` value will be filled in manually after key generation.

### Step 4: Extend settings to support `auto_update`

The current settings model only handles LLM config. We need to add an `auto_update` boolean.

**Option A — Separate settings file**: Add a new `app_settings.json` alongside `settings.json` with `{ "auto_update": false }`. New Rust struct `AppSettings`, new load/save functions, new Tauri commands (`get_app_settings`, `update_app_settings`), new `AppSettingsState`.

**Option B — Extend existing settings file**: Add `auto_update` to the existing `LlmConfig` struct (rename it to `AppConfig` or similar). Backward-compatible via `#[serde(default)]`.

Recommendation: **Option A** (separate file) keeps concerns clean — LLM config and app preferences are unrelated. The existing settings.json may already exist on user machines with only LLM fields.

Files to create/modify:
- Create `tauri_src/src/utils/app_settings.rs` — `AppSettings { auto_update: bool }` with load/save
- Create `tauri_src/src/commands/app_settings_commands.rs` — `get_app_settings`, `update_auto_update`
- Modify `tauri_src/src/lib.rs` — register new state and commands
- Modify `tauri_src/src/commands/mod.rs` — add module

### Step 5: Add update check logic in the frontend

Create `src/renderer/utils/updater.js`:
```js
import { check } from '@tauri-apps/plugin-updater';

export async function checkForUpdate() {
  try {
    const update = await check();
    if (update?.available) {
      return {
        available: true,
        version: update.version,
        notes: update.body,
        download: () => update.downloadAndInstall(),
      };
    }
    return { available: false };
  } catch (e) {
    console.error('Update check failed:', e);
    return { available: false };
  }
}
```

### Step 6: Add update notification banner in `App.jsx`

On mount, call `checkForUpdate()`. If an update is available:
- Check `auto_update` setting via `window.backendAPI.getAppSettings()`
- If `auto_update` is **off**: render a dismissible banner at the top of the app:
  `"Update available — v{version}. [Install Now] [Dismiss]"`
- If `auto_update` is **on**: automatically call `download()`, show a progress/restart banner

Add state to `App.jsx`:
```js
const [updateInfo, setUpdateInfo] = useState(null);
const [updateDismissed, setUpdateDismissed] = useState(false);
```

Create a small `UpdateBanner` component (inline or separate file) that renders above the current page content.

### Step 7: Add auto-update toggle to `SettingsPage.jsx`

Add a new "Updates" section below the existing "LLM Provider" section:
```jsx
<div className="settings-section">
  <h3 className="settings-section-title">Updates</h3>
  <label>
    <input type="checkbox" checked={autoUpdate} onChange={...} />
    Automatically install updates
  </label>
</div>
```

Wire it to the new `getAppSettings` / `updateAutoUpdate` backend commands via `tauriAPI.js`.

### Step 8: Add backend API wrappers in `tauriAPI.js`

```js
getAppSettings: () => invoke('get_app_settings'),
updateAutoUpdate: (autoUpdate) => invoke('update_auto_update', { autoUpdate }),
```

### Step 9: Update CI workflow to generate `latest.json`

Modify `.github/workflows/release.yml`:

1. **Build step**: Add `TAURI_SIGNING_PRIVATE_KEY` env var (from GitHub secret) to the `tauri-apps/tauri-action` step so `.sig` files are generated.

2. **Release step**: After building, read the `.sig` file content and generate `latest.json`:
```json
{
  "version": "0.5.0",
  "notes": "changelog content",
  "pub_date": "2024-01-01T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "<contents of .sig file>",
      "url": "https://github.com/ethanbond64/proveall/releases/download/v0.5.0/ProveAll_0.5.0_aarch64.app.tar.gz"
    }
  }
}
```

3. Upload `latest.json` as a release asset alongside `.dmg` and `.app.tar.gz`.

4. Upload the `.sig` file as a release asset too.

### Step 10: Update Tauri capabilities

Add updater permissions to `tauri_src/capabilities/default.json` (or the relevant capability file):
```json
"permissions": [
  "updater:default"
]
```

## File Change Summary

| File | Action |
|------|--------|
| `tauri_src/Cargo.toml` | Add `tauri-plugin-updater` dependency |
| `package.json` | Add `@tauri-apps/plugin-updater` dependency |
| `tauri_src/src/lib.rs` | Register updater plugin, new state, new commands |
| `tauri_src/tauri.conf.json` | Add `plugins.updater` config |
| `tauri_src/src/utils/app_settings.rs` | **New** — AppSettings struct, load/save |
| `tauri_src/src/utils/mod.rs` | Add `app_settings` module |
| `tauri_src/src/commands/app_settings_commands.rs` | **New** — get/update commands |
| `tauri_src/src/commands/mod.rs` | Add `app_settings_commands` module |
| `tauri_src/capabilities/default.json` | Add updater permissions |
| `src/renderer/utils/updater.js` | **New** — update check logic |
| `src/renderer/tauriAPI.js` | Add `getAppSettings`, `updateAutoUpdate` |
| `src/renderer/App.jsx` | Add update check on mount, render UpdateBanner |
| `src/renderer/pages/SettingsPage.jsx` | Add auto-update toggle |
| `src/renderer/styles.css` | Add banner styles |
| `.github/workflows/release.yml` | Add signing key env, generate `latest.json`, upload assets |

## Manual Steps (Not Covered)

1. Generate Tauri signing keypair: `npx tauri signer generate -w ~/.tauri/proveall.key`
2. Add `TAURI_SIGNING_PRIVATE_KEY` to GitHub repo secrets
3. Replace `PLACEHOLDER_UNTIL_KEY_IS_GENERATED` in `tauri.conf.json` with the public key
4. Test end-to-end with a real release
