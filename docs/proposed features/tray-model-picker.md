# Proposed Feature: Tray Model Picker

## Goal
Allow changing Whisper model directly from the system tray menu, without manually editing `config.ini`.

## Feasibility
Yes. The app already uses a tray menu (`Platform.SystemTrayIcon` with `Platform.Menu` in `src/qml/Main.qml`), so adding model-selection menu items is straightforward.

Tray UI is limited (menu items, separators, simple toggles), but this feature fits perfectly in that model.

## UX Proposal
- Add a `Model` section in the tray menu.
- Show selectable options like:
  - `tiny.en (fastest)`
  - `base.en (default)`
  - `small.en`
  - `large-v3-turbo` (if enabled)
- Indicate current model with a checkmark/prefix.
- On selection:
  - Persist to config
  - Show status: `Switching model...`
  - Restart service/app to load the new model cleanly

## Why Restart Is Best (MVP)
Current service loads settings + model once at startup (`src/service.rs`) and keeps a long-lived `Transcriber`. A clean restart avoids risky hot-swap state transitions in the first version.

## Implementation Plan
1. Add model metadata list in Rust (id + label + optional notes).
2. Expose current model and selectable models through the Qt bridge.
3. Add tray menu items in `src/qml/Main.qml` for model selection.
4. Add `set_model(model_id)` invokable in bridge:
   - update config file (`model = ...`)
   - trigger graceful restart.
5. If model is missing, existing download flow already handles it on next startup.

## Optional Enhancements
- `Download first` behavior before restart for better UX.
- Separate `Recommended` and `Advanced` models.
- Display approximate model size + speed hints.
- Add quantized model variants once download source is expanded.

## Risks / Notes
- Some tray implementations handle complex menus differently; keep first version simple.
- Large model switch can incur first-run download delay.
- We should avoid switching while actively recording/transcribing.

## Acceptance Criteria
- User can choose model from tray menu.
- Chosen model persists to config.
- App restarts and uses new model.
- If model is not cached, it downloads and then runs normally.
