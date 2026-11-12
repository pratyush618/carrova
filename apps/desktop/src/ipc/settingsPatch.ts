import type { Settings } from '../bindings/Settings';

/**
 * Changed settings only. The bridge applies them to the saved settings (a
 * JSON merge patch), so a change made elsewhere meanwhile (the tray's pause)
 * stays. Objects merge field by field; `null` clears an optional field.
 * Background settings change through `setBackground`.
 */
export type SettingsPatch = {
  readonly [K in Exclude<keyof Settings, 'background'>]?: Settings[K] extends object
    ? Partial<Settings[K]>
    : Settings[K];
};

/** `settings` with `patch` applied, as the bridge applies it. */
export const withPatch = (settings: Settings, patch: SettingsPatch): Settings => ({
  ...settings,
  ...patch,
  alerts: { ...settings.alerts, ...patch.alerts },
  move_defaults: { ...settings.move_defaults, ...patch.move_defaults },
  nearby: { ...settings.nearby, ...patch.nearby },
});
