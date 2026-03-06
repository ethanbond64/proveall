# Plan: Tauri Auto-Updater for ProveAll

**Prerequisite:** This plan assumes the settings API refactor (see `PLAN-settings-refactor.md`) is complete. The `AppSettings` struct already includes `auto_update: bool`, and the generic `get_settings`/`set_settings`/`reset_settings` API is in place.

## Overview

Add auto-update functionality using `tauri-plugin-updater` so users are notified when a new version is available, with an optional setting to auto-install updates.

## Decisions

- **Update endpoint**: Static `latest.json` file hosted on GitHub Releases
- **Update logic**: JavaScript/frontend-driven via `@tauri-apps/plugin-updater` JS API
- **Key generation**: Manual (not covered by this plan)

## Current State (Post Settings Refactor)

- **Version**: 0.4.1, tracked in 4 places (package.json, tauri.conf.json, Cargo.toml, CHANGELOG.md)
- **Releases**: Automated via GitHub Actions on push to `main`. Builds macOS aarch64 `.dmg` and `.app.tar.gz`, creates a GitHub Release with version tag.
- **Settings**: `AppSettings` struct with `llm_command`, `llm_args`, `auto_update` fields. Generic `get_settings`/`set_settings`/`reset_settings` API. Settings toggle for `auto_update` can be added purely in the frontend.
- **GitHub repo**: `https://github.com/ethanbond64/proveall`

## Implementation Steps

### Step 1: Install plugin dependencies

**Cargo** — add to `tauri_src/Cargo.toml`:
```toml
tauri-plugin-updater = "2"
```

**npm**:
```
npm install @tauri-apps/plugin-updater
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

### Step 4: Update Tauri capabilities

Add updater permissions to `tauri_src/capabilities/default.json`:
```json
"permissions": ["dialog:default", "updater:default"]
```

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
- Read `auto_update` from settings via `window.backendAPI.getSettings()`
- If `auto_update` is **off**: render a dismissible banner at the top of the app:
  `"Update available — v{version}. [Install Now] [Dismiss]"`
- If `auto_update` is **on**: automatically call `download()`, show a progress/restart banner

Add state to `App.jsx`:
```js
const [updateInfo, setUpdateInfo] = useState(null);
const [updateDismissed, setUpdateDismissed] = useState(false);
```

Create a small `UpdateBanner` component (inline or separate file) rendered above the current page content.

### Step 7: Add auto-update toggle to `SettingsPage.jsx`

Add a new "Updates" section below the existing "LLM Provider" section:
```jsx
<div className="settings-section">
  <h3 className="settings-section-title">Updates</h3>
  <label>
    <input type="checkbox" checked={autoUpdate} onChange={handleAutoUpdateToggle} />
    Automatically install updates
  </label>
</div>
```

The toggle reads/writes `auto_update` via the existing `getSettings`/`setSettings` API — no new backend work needed.

### Step 8: Update CI workflow to generate `latest.json`

Modify `.github/workflows/release.yml`:

1. **Build step**: Add `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` env vars (from GitHub secrets) to the `tauri-apps/tauri-action` step so `.sig` files are generated alongside `.app.tar.gz`.

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

## File Change Summary

| File | Action |
|------|--------|
| `tauri_src/Cargo.toml` | Add `tauri-plugin-updater` dependency |
| `package.json` | Add `@tauri-apps/plugin-updater` dependency |
| `tauri_src/src/lib.rs` | Register updater plugin |
| `tauri_src/tauri.conf.json` | Add `plugins.updater` config |
| `tauri_src/capabilities/default.json` | Add `updater:default` permission |
| `src/renderer/utils/updater.js` | **New** — update check logic |
| `src/renderer/App.jsx` | Add update check on mount, render UpdateBanner |
| `src/renderer/pages/SettingsPage.jsx` | Add auto-update toggle (uses existing settings API) |
| `src/renderer/styles.css` | Add banner styles |
| `.github/workflows/release.yml` | Add signing key env, generate + upload `latest.json` and `.sig` |

## Manual Steps (Not Covered)

1. Generate Tauri signing keypair: `npx tauri signer generate -w ~/.tauri/proveall.key`
2. Add `TAURI_SIGNING_PRIVATE_KEY` (and password if set) to GitHub repo secrets
3. Replace `PLACEHOLDER_UNTIL_KEY_IS_GENERATED` in `tauri.conf.json` with the public key
4. Test end-to-end with a real release
