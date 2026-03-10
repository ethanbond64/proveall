import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

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

export { relaunch };
