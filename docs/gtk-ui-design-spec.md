# Escucha GTK4/libadwaita UI Design Specification

## Overview

Escucha is a hold-to-talk speech-to-text app. This spec redesigns its UI from basic egui to a polished GTK4 + libadwaita interface that feels native to GNOME.

**Design philosophy**: Calm when idle, alive when recording, minimal always.

---

## 1. Window Structure

### AdwApplicationWindow
- **Default size**: 420 x 520 px
- **Minimum size**: 360 x 400 px (libadwaita default min is 360x200)
- **Title**: "Escucha"
- **App ID**: `io.github.escucha` (or similar reverse-domain)
- **Resizable**: Yes

### Widget Hierarchy

```
AdwApplicationWindow
  └── AdwToolbarView
        ├── [top] AdwHeaderBar
        │         ├── [title-widget] AdwWindowTitle ("Escucha", subtitle: device label)
        │         └── [end] GtkMenuButton (hamburger menu, optional/future)
        └── [content] AdwToastOverlay
                       └── GtkBox (vertical, spacing: 0)
                             ├── StatusArea (custom, centered)
                             │     ├── Status Icon (large, 64px)
                             │     ├── Status Label (title-2)
                             │     └── Status Detail (dim-label)
                             ├── GtkSeparator
                             └── TranscriptionArea
                                   ├── Section Header ("Last transcription")
                                   └── GtkScrolledWindow
                                         └── GtkLabel (selectable, wrapping)
```

---

## 2. Header Bar

### AdwHeaderBar
- Use default window controls (close/minimize/maximize on the appropriate side based on system settings).
- **Title widget**: `AdwWindowTitle`
  - **Title**: "Escucha"
  - **Subtitle**: The active device label (e.g., "AT Translated Set 2 keyboard")
    - Strip the `/dev/input/eventN - ` prefix; show only the human-readable device name
    - While detecting: show "Detecting device..." as subtitle in italics
    - If no device / stopped: show no subtitle (empty string)

### Future: Hamburger menu
- Not required for initial implementation, but leave space for a `GtkMenuButton` with `open-menu-symbolic` icon at the end of the header bar for future settings (model selection, hotkey config, etc.).

---

## 3. Status Area (Center of Window)

This is the hero area of the app. It communicates the current state at a glance.

### Layout
```
AdwClamp (maximum-size: 360)
  └── GtkBox (vertical, halign: center, valign: center, spacing: 12)
        ├── StatusIcon (GtkImage, 64px, centered)
        ├── StatusLabel (GtkLabel, .title-2, centered)
        └── StatusDetail (GtkLabel, .dim-label, centered, optional)
```

- The status area should be vertically centered and take flexible space (vexpand: true).
- Use `AdwClamp` with `maximum-size: 360` to keep it centered and not too wide.
- The `GtkBox` inside should have `halign: center` and `valign: center`.

### Vertical expansion
- The status area box should have `vexpand: true` to push it into the center of the available space above the transcription section. This creates a clean visual hierarchy: status dominates the upper space, transcription sits at the bottom.

---

## 4. Visual States

Each state has a distinct icon, label, CSS class, and feel:

### Stopped
- **Icon**: `microphone-disabled-symbolic` (64px)
- **Icon CSS**: `.dim-label` (dimmed)
- **Label**: "Stopped"
- **Label CSS**: `.dim-label`
- **Detail**: (none)
- **Feel**: Subdued, inactive. Everything is gray/dimmed.

### Starting
- **Icon**: `AdwSpinner` (replaces the GtkImage, 32px)
- **Label**: "Starting..."
- **Label CSS**: (default color)
- **Detail**: Shows status_msg (e.g., "Downloading model 'base.en'...", "Loading model...")
- **Detail CSS**: `.dim-label`
- **Feel**: Active but patient. The spinner communicates progress.

### Ready
- **Icon**: `audio-input-microphone-symbolic` (64px)
- **Icon CSS**: `.success` (green tint via Adwaita named color)
- **Label**: "Ready"
- **Label CSS**: `.success`
- **Detail**: "Hold Right Ctrl to speak" (or whatever the configured hotkey is)
- **Detail CSS**: `.dim-label`
- **Feel**: Calm, inviting, ready. The green icon says "I'm listening for your command."

### Recording
- **Icon**: `microphone-sensitivity-high-symbolic` (64px)
- **Icon CSS class**: `.error` + `.recording-pulse` (custom CSS animation)
- **Label**: "Recording..."
- **Label CSS**: `.error` (red)
- **Detail**: (none, or a subtle "Release to transcribe")
- **Detail CSS**: `.dim-label`
- **Pulsing animation** (CSS):
  ```css
  @keyframes recording-pulse {
      0%, 100% { opacity: 1.0; }
      50% { opacity: 0.4; }
  }
  .recording-pulse {
      animation: recording-pulse 1.2s ease-in-out infinite;
  }
  ```
- **Feel**: Alive, urgent, active. The pulsing red icon immediately communicates "I'm recording right now." This is the most visually dynamic state.

### Transcribing
- **Icon**: `AdwSpinner` (replaces the GtkImage, 32px)
- **Label**: "Transcribing..."
- **Label CSS**: (default color)
- **Detail**: (none)
- **Feel**: Brief processing state. Similar to Starting but typically very short-lived.

### Stopping
- **Icon**: `AdwSpinner` (replaces the GtkImage, 32px)
- **Label**: "Stopping..."
- **Label CSS**: `.dim-label`
- **Detail**: (none)
- **Feel**: Winding down. Brief transitional state.

### Implementation Notes for State Transitions

When switching states, swap the icon widget between `GtkImage` and `AdwSpinner` as needed. The simplest approach:

- Keep a `GtkStack` with two children: a `GtkImage` (named "icon") and an `AdwSpinner` (named "spinner")
- Switch the visible child based on state
- Update the GtkImage's icon-name and CSS classes when switching to icon-based states
- Transition type: `GTK_STACK_TRANSITION_TYPE_CROSSFADE` with duration ~150ms for smooth state changes

---

## 5. Transcription Area (Bottom of Window)

### Layout
```
GtkBox (vertical, spacing: 0)
  ├── GtkBox (horizontal, margin: 12px horizontal)
  │     └── GtkLabel ("Last transcription", .heading, .dim-label)
  ├── GtkSeparator (horizontal, margin: 0)
  └── GtkScrolledWindow (vexpand: false, min-height: 120, max preferred ~200)
        └── AdwClamp (maximum-size: 600)
              └── GtkLabel (selectable: true, wrap: true, .body, margin: 12)
```

### Transcription Label
- Use `GtkLabel` with `selectable: true` and `wrap: true` (not a GtkTextView - we just need read-only selectable text)
- `xalign: 0` (left-aligned)
- `wrap-mode: WORD_CHAR`
- CSS class: `.body`
- Margin: 12px on all sides inside the clamp

### Empty State
- When no transcription yet, show placeholder text:
  - Text: "Hold Right Ctrl and speak..." (use configured hotkey name)
  - CSS: `.dim-label`

### Populated State
- Show the full transcription text
- Left-aligned, wrapping naturally
- Selectable so the user can copy portions

### Scrolling
- `GtkScrolledWindow` with:
  - `vscrollbar-policy: AUTOMATIC`
  - `hscrollbar-policy: NEVER`
  - Min content height: 120px
  - The scroll window should get a reasonable portion of the window but not dominate. Use a preferred natural height around 150-200px. Don't set vexpand on this area - let the status area take the flexible space.

### Visual Separation
- A `GtkSeparator` between the status area and transcription area
- This creates a clear visual boundary

---

## 6. Error Handling

### Use AdwToast for Transient Errors
- When `ServiceMessage::Error` is received, display an `AdwToast`:
  - **Title**: The error message text
  - **Timeout**: 5 seconds (longer than default, errors deserve attention)
  - **Priority**: `ADW_TOAST_PRIORITY_HIGH` (ensures it's shown even if another toast is visible)

### Why Toast instead of inline error
- Errors in Escucha are typically transient (transcription failure, audio device hiccup)
- A toast doesn't clutter the UI permanently
- The user can dismiss it
- It doesn't fight for space with the clean status/transcription layout

### Implementation
```rust
// When error received:
let toast = adw::Toast::new(&error_msg);
toast.set_timeout(5);
toast.set_priority(adw::ToastPriority::High);
toast_overlay.add_toast(toast);
```

---

## 7. Typography

| Element | CSS Class | Size | Weight |
|---|---|---|---|
| Window title (in header bar) | AdwWindowTitle default | System | Bold |
| Header bar subtitle (device) | AdwWindowTitle default | System small | Normal |
| Status label ("Ready", "Recording...") | `.title-2` | ~20px | Bold |
| Status detail ("Hold Right Ctrl...") | `.dim-label` | Body | Normal |
| Section header ("Last transcription") | `.heading` + `.dim-label` | ~15px | Bold |
| Transcription text | `.body` | Body (~14px) | Normal |
| Placeholder text | `.dim-label` | Body | Normal |

All sizes follow Adwaita defaults - don't hardcode pixel sizes. Use the CSS classes and let the system scale appropriately.

---

## 8. Spacing & Margins

| Location | Value |
|---|---|
| Status area top margin | 24px |
| Spacing between icon and status label | 12px |
| Spacing between status label and detail | 4px |
| Space above separator | 12px |
| Section header horizontal padding | 12px |
| Section header vertical padding | 8px top, 0 bottom |
| Transcription text margin (inside clamp) | 12px all sides |
| AdwClamp max-width (status area) | 360px |
| AdwClamp max-width (transcription area) | 600px |

---

## 9. Custom CSS

Create a `style.css` resource file:

```css
/* Recording pulse animation */
@keyframes recording-pulse {
    0%, 100% { opacity: 1.0; }
    50% { opacity: 0.4; }
}

.recording-pulse {
    animation: recording-pulse 1.2s ease-in-out infinite;
}

/* Status icon sizing */
.status-icon {
    -gtk-icon-size: 64px;
}

/* Make the transcription area have a slightly different background
   to visually separate it from the status area */
.transcription-area {
    background-color: var(--view-bg-color);
}
```

Load via `GtkCssProvider` at `GTK_STYLE_PROVIDER_PRIORITY_APPLICATION`.

---

## 10. Color Mapping (State to Adwaita Colors)

| State | Icon Color | Text Color | CSS Variable |
|---|---|---|---|
| Stopped | Dimmed | Dimmed | (opacity via `.dim-label`) |
| Starting | N/A (spinner) | Default | (default fg) |
| Ready | Green | Green | `@success_color` via `.success` |
| Recording | Red + pulse | Red | `@error_color` via `.error` |
| Transcribing | N/A (spinner) | Default | (default fg) |
| Stopping | N/A (spinner) | Dimmed | (opacity via `.dim-label`) |

Use Adwaita CSS classes (`.success`, `.error`, `.dim-label`) rather than hardcoded colors. This ensures proper dark/light theme support automatically.

---

## 11. Dark Mode Support

- Fully automatic via libadwaita. By using CSS variables and Adwaita CSS classes, dark mode works out of the box.
- Do NOT set any explicit color scheme preference - let the system decide.
- The `.success`, `.error`, `.dim-label` classes all adapt to dark mode automatically.

---

## 12. State Transition Summary

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│ Stopped  │────▶│ Starting │────▶│  Ready   │
│ (dim)    │     │ (spinner)│     │ (green)  │
└──────────┘     └──────────┘     └────┬─────┘
                                       │
                                       ▼
                                 ┌──────────┐
                                 │Recording │
                                 │(red pulse)│
                                 └────┬─────┘
                                       │
                                       ▼
┌──────────┐     ┌──────────────┐
│ Stopping │◀────│ Transcribing │──────▶ Ready (loop)
│ (spinner)│     │  (spinner)   │
└──────────┘     └──────────────┘
```

---

## 13. Responsive Behavior

- At narrow widths (< 360px), `AdwClamp` gracefully shrinks content.
- No breakpoints needed for this simple app - the layout is inherently single-column and responsive.
- Text wrapping handles long transcriptions naturally.
- The header bar subtitle truncates with ellipsis if the device name is very long.

---

## 14. Complete Widget Tree (for implementation reference)

```xml
<!-- Pseudo-blueprint / XML structure -->
AdwApplicationWindow {
    title: "Escucha"
    default-width: 420
    default-height: 520

    content: AdwToolbarView {
        [top] AdwHeaderBar {
            title-widget: AdwWindowTitle {
                title: "Escucha"
                subtitle: bind device_label
            }
            /* [end] GtkMenuButton { icon-name: "open-menu-symbolic" }  -- future */
        }

        content: AdwToastOverlay#toast_overlay {
            child: GtkBox.vertical {

                /* === Status Area === */
                AdwClamp {
                    maximum-size: 360
                    vexpand: true

                    GtkBox.vertical {
                        halign: center
                        valign: center
                        spacing: 12

                        GtkStack#status_icon_stack {
                            transition-type: crossfade
                            transition-duration: 150

                            GtkImage#status_icon {
                                name: "icon"
                                icon-name: "audio-input-microphone-symbolic"
                                pixel-size: 64
                                css-classes: ["status-icon"]
                            }

                            AdwSpinner#status_spinner {
                                name: "spinner"
                                width-request: 32
                                height-request: 32
                            }
                        }

                        GtkLabel#status_label {
                            label: "Ready"
                            css-classes: ["title-2"]
                        }

                        GtkLabel#status_detail {
                            label: "Hold Right Ctrl to speak"
                            css-classes: ["dim-label"]
                            visible: true  /* hide when empty */
                        }
                    }
                }

                /* === Separator === */
                GtkSeparator.horizontal {}

                /* === Transcription Area === */
                GtkBox.vertical.transcription-area {

                    GtkLabel {
                        label: "Last transcription"
                        css-classes: ["heading", "dim-label"]
                        xalign: 0
                        margin-start: 12
                        margin-end: 12
                        margin-top: 8
                        margin-bottom: 4
                    }

                    GtkScrolledWindow {
                        vscrollbar-policy: automatic
                        hscrollbar-policy: never
                        min-content-height: 120

                        AdwClamp {
                            maximum-size: 600

                            GtkLabel#transcription_label {
                                label: "Hold Right Ctrl and speak..."
                                css-classes: ["body", "dim-label"]
                                selectable: true
                                wrap: true
                                wrap-mode: word-char
                                xalign: 0
                                yalign: 0
                                margin-start: 12
                                margin-end: 12
                                margin-top: 8
                                margin-bottom: 12
                            }
                        }
                    }
                }
            }
        }
    }
}
```

---

## 15. Implementation Notes

### State Update Logic (Pseudocode)

```rust
fn update_ui_for_state(&self, status: ServiceStatus) {
    match status {
        Stopped => {
            self.icon_stack.set_visible_child_name("icon");
            self.status_icon.set_icon_name(Some("microphone-disabled-symbolic"));
            self.status_icon.set_css_classes(&["status-icon", "dim-label"]);
            self.status_icon.remove_css_class("recording-pulse");
            self.status_label.set_text("Stopped");
            self.status_label.set_css_classes(&["title-2", "dim-label"]);
            self.status_detail.set_visible(false);
        }
        Starting => {
            self.icon_stack.set_visible_child_name("spinner");
            self.status_label.set_text("Starting...");
            self.status_label.set_css_classes(&["title-2"]);
            // status_detail updated via StatusMsg
            self.status_detail.set_visible(!self.status_msg.is_empty());
        }
        Ready => {
            self.icon_stack.set_visible_child_name("icon");
            self.status_icon.set_icon_name(Some("audio-input-microphone-symbolic"));
            self.status_icon.set_css_classes(&["status-icon", "success"]);
            self.status_icon.remove_css_class("recording-pulse");
            self.status_label.set_text("Ready");
            self.status_label.set_css_classes(&["title-2", "success"]);
            self.status_detail.set_text("Hold Right Ctrl to speak");
            self.status_detail.set_visible(true);
        }
        Recording => {
            self.icon_stack.set_visible_child_name("icon");
            self.status_icon.set_icon_name(Some("microphone-sensitivity-high-symbolic"));
            self.status_icon.set_css_classes(&["status-icon", "error", "recording-pulse"]);
            self.status_label.set_text("Recording...");
            self.status_label.set_css_classes(&["title-2", "error"]);
            self.status_detail.set_text("Release to transcribe");
            self.status_detail.set_visible(true);
        }
        Transcribing => {
            self.icon_stack.set_visible_child_name("spinner");
            self.status_icon.remove_css_class("recording-pulse");
            self.status_label.set_text("Transcribing...");
            self.status_label.set_css_classes(&["title-2"]);
            self.status_detail.set_visible(false);
        }
        Stopping => {
            self.icon_stack.set_visible_child_name("spinner");
            self.status_label.set_text("Stopping...");
            self.status_label.set_css_classes(&["title-2", "dim-label"]);
            self.status_detail.set_visible(false);
        }
    }
}
```

### Polling for Messages
- Use `glib::timeout_add_local(Duration::from_millis(100), ...)` to poll the `mpsc::Receiver<ServiceMessage>` channel from the GTK main loop, similar to the current egui approach.
- Alternatively, use `glib::MainContext::channel()` for a GLib-native async channel that integrates directly with the GTK event loop (preferred approach - avoids polling entirely).

### Transcription Update Logic
```rust
fn update_transcription(&self, text: &str) {
    if text.is_empty() {
        self.transcription_label.set_text("Hold Right Ctrl and speak...");
        self.transcription_label.add_css_class("dim-label");
    } else {
        self.transcription_label.set_text(text);
        self.transcription_label.remove_css_class("dim-label");
    }
}
```

---

## 16. Summary of Key Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Window type | AdwApplicationWindow + AdwToolbarView | Standard libadwaita app pattern |
| Device display | Header bar subtitle | Keeps it visible but out of the way |
| Status display | Large centered icon + label | Hero pattern, clear at a glance |
| Recording feedback | Pulsing red icon animation | Dynamic, immediately noticeable |
| Spinner states | AdwSpinner via GtkStack | Smooth crossfade transitions |
| Error display | AdwToast | Non-intrusive, auto-dismissing |
| Transcription | Selectable GtkLabel in ScrolledWindow | Simple, read-only, copyable |
| Colors | Adwaita CSS classes only | Automatic dark mode support |
| Layout | AdwClamp for content width | Proper responsive behavior |
