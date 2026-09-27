# egui capabilities and Windows integration crates

Research for issue #11.
Question: what can `egui`/`eframe` do today for a native Discord client on Windows, and which crates fill the gaps?

All versions were read from the crates.io API on 2026-09-21, and all behavioural claims were checked against the crate source or the released docs.rs API, not against blog posts or tutorials.

## Version baseline

The egui workspace is at **0.36.2**, published 2026-09-08, MSRV 1.95, dual MIT/Apache-2.0.
That covers `egui`, `eframe` and `egui_extras`, which are released together from the same repository and must always be pinned to the same version.

| Crate | Version | Published | MSRV | License | Maintenance |
| --- | --- | --- | --- | --- | --- |
| `egui` / `eframe` / `egui_extras` | 0.36.2 | 2026-09-08 | 1.95 | MIT OR Apache-2.0 | Very active, funded by Rerun, roughly one minor release every six to eight weeks |
| `egui_commonmark` | 0.25.0 | 2026-08-05 | 1.95 | MIT OR Apache-2.0 | Active third party (lampsitter), tracks egui releases within days |
| `notify-rust` | 4.18.0 | 2026-06-16 | 1.89 | MIT OR Apache-2.0 | Active, Linux-first |
| `tauri-winrt-notification` | 0.8.1 | 2026-07-17 | 1.82 | MIT OR Apache-2.0 | Active, maintained by the Tauri org |
| `tray-icon` | 0.25.1 | 2026-09-16 | 1.90 | MIT OR Apache-2.0 | Active, maintained by the Tauri org |
| `keyring` | 4.2.0 | 2026-08-29 | 1.88 | MIT OR Apache-2.0 | Active, recently restructured |
| `dark-light` | 3.0.0 | 2026-08-12 | 1.78 | MIT/Apache-2.0 | Active |
| `arboard` | 3.6.1 | 2025-08-23 | 1.71 | MIT OR Apache-2.0 | Stable, maintained by 1Password, slow release cadence |
| `image` | 0.25.10 | 2026-03-10 | — | MIT OR Apache-2.0 | Active; pulled in transitively by `egui_extras` |

A note that shapes several decisions below: a number of features FastCord wants landed on egui `main` after 0.36.2 and are **not** in a released crate yet.
I verified this by querying docs.rs for 0.36.2 directly: `egui::Event::PasteImage` and `egui::CompletionPopup` do not exist there (the `Event` enum on docs.rs for 0.36.2 has only `Paste(String)`, and `struct.CompletionPopup.html` returns 404), while both are present in the `main` branch source.
They will presumably ship in 0.37.

## Capability by capability

| Capability | Native egui 0.36.2 | Crate needed | Gap that stays |
| --- | --- | --- | --- |
| Rich text in a message list | `LayoutJob` / `RichText` with per-range colour, style, background, link underline | — | No inline widgets mid-paragraph; a spoiler or a mention pill has to be a separate atom, not a text range |
| Inline images | `egui::Image` with the loader pipeline, `egui_extras::install_image_loaders` | `egui_extras` with `image`, `webp`, `gif`, `http` features | Loaders are global per `Context` and keyed by URI string; cache eviction is manual (`forget`, `forget_all`) |
| Markdown rendering | — | `egui_commonmark` 0.25 | CommonMark + a GitHub subset; Discord's dialect (spoilers, `<@id>` mentions, custom emoji, timestamps) is not CommonMark and will need our own parser or a pre-pass |
| Animated GIF / WebP | Yes, via `egui_extras` `gif` / `webp` loaders | `egui_extras` | Decode control is effectively absent — see below |
| Virtualized long list | `ScrollArea::show_rows` (uniform height), `ScrollArea::show_viewport` (manual) | — | `show_rows` demands one fixed row height, which a message list does not have; we must build our own height-cache virtualizer on `show_viewport` |
| Table layout | `egui_extras::TableBuilder` | `egui_extras` | Column-oriented, wrong shape for a chat transcript |
| Multiline editor | `TextEdit::multiline`, undo, IME, `event_filter` (new in 0.36.2) | — | Fine |
| Autocomplete popup | `egui::Popup` (`from_response`, `at_position`, `close_behavior`) | — | The purpose-built `CompletionPopup` / `Suggestion` API exists on `main` but not in 0.36.2; until 0.37 we wire `Popup::at_position` to the caret ourselves |
| Clipboard image paste | `Event::Paste(String)` only — text | `arboard` (`Clipboard::get_image` → `ImageData`) | egui 0.36.2 surfaces no image paste event; we read the clipboard ourselves on Ctrl+V. `Event::PasteImage` is on `main` for 0.37 |
| Clipboard text copy/paste | `Event::Copy` / `Cut` / `Paste`, `OutputCommand::CopyText` | — (eframe already uses arboard internally) | Fine |
| System notifications | — | `notify-rust` 4.18 or `tauri-winrt-notification` 0.8 | Needs a registered AppUserModelID; see below |
| Tray icon | — | `tray-icon` 0.25 | Requires a running event loop on the creating thread on Windows |
| Credential storage | — | `keyring` 4.2 | Crate was restructured in v4; see below |
| Light/dark/system theme | `ThemePreference::{Dark, Light, System}`, `RawInput::system_theme`, and since 0.36.0 the OS titlebar follows the app theme | — (`dark-light` only if we need the OS theme outside a window) | Fine; `dark-light` is probably not needed |
| Colour emoji | Bundled fonts are `NotoEmoji-Regular` and `emoji-icon-font`, both monochrome | Custom font via `Context::set_fonts`, or render emoji as images | No colour emoji out of the box; Discord custom emoji are images anyway, so unicode emoji are the open question |

### Animated images: the real constraint

This is the one place where egui's behaviour is much more rigid than the API surface suggests, so it is worth stating precisely.

When `Image` is given a URI ending in `.gif` or `.webp` (or bytes with a GIF/WebP magic header), it rewrites the URI to `"{uri}#{frame_index}"` and asks the loader for that single frame.
The frame index comes from `animated_image_frame_index`, which reads `ctx.input(|i| i.time)` — wall-clock time since app start — takes it modulo the total animation duration, walks the per-frame durations, and calls `ctx.request_repaint_after` for the remaining time on the current frame.

Three consequences:

1. **There is no play/pause/seek API.** Animation position is a pure function of global time. To stop a GIF you must stop drawing it, or bypass `Image` entirely and call `ctx.try_load_image` yourself with a hand-built `"{uri}#{n}"` URI. `decode_animated_image_uri` is public; the encoding counterpart is private, but the format is just `uri`, `#`, index.
2. **Decoding is eager and complete.** `AnimatedImage::load_gif` runs `image::codecs::gif::GifDecoder` over every frame up front and keeps each one as an `Arc<ColorImage>` — uncompressed RGBA in RAM, plus whatever the renderer uploads. A 500-frame GIF is fully resident the moment it is first shown. The loader exposes `byte_size` and `forget`, so budget enforcement is ours to write.
3. **Every visible animation forces repaints.** Each animated image schedules a repaint at its next frame boundary, and egui repaints the whole UI. Several GIFs in view means the app is redrawing continuously at the fastest GIF's rate.

For a client that wants "animate on hover" or "pause when off-screen", the practical route is a thin wrapper around `try_load_image` with our own frame clock per message, rather than `Image::new(uri)`.

### Performance pitfalls with large UIs

egui is immediate mode: layout runs in full every frame for everything in the tree.
The upstream README is explicit that "having a very large UI in a scroll area (with very long scrollback) can be slow, as the content needs to be laid out each frame", and recommends laying out only what is in view.
For a chat client that is the central architectural constraint, not a tuning detail.

Text layout is memoized. `epaint`'s `GalleyCache` hashes the whole `LayoutJob` plus `pixels_per_point` and reuses the resulting `Galley`; entries not touched during a frame are evicted at end of pass. So a stable visible message stays cheap, but the hash of its full layout job is recomputed every frame, and scrolling a message off-screen throws its galley away.

Other items worth designing around:

- egui only repaints on interaction or animation, so an idle client costs nothing — but any visible GIF, spinner or animated highlight removes that property.
- Layout that depends on size resolves one frame late ("first-frame jitter"). `Context::request_discard` buys a second pass at the cost of a second full layout.
- `max_texture_side` comes from the GPU (`GL_MAX_TEXTURE_SIZE`) and defaults to a conservative 2048 when the integration does not report one. Large attachments must be downscaled before upload.
- Widget state is keyed by `Id`; a virtualized list that reuses row positions needs stable per-message ids (`ui.push_id(message_id)`), or scroll state and text selection will jump.

### Notifications on Windows

`notify-rust` is a cross-platform façade that delegates to `winrt-notification` on Windows; its own README describes Windows and macOS as secondary to its Linux/BSD origin.
`tauri-winrt-notification` is the direct WinRT toast wrapper: Windows 10 and 8.1 tested, explicitly broken on Windows 7, and on 8.1 only one image per toast survives.

The operational catch is the AppUserModelID. `Toast::new` takes one, and the crate's own example falls back to `Toast::POWERSHELL_APP_ID` — a borrowed identity that works for demos but shows the wrong name and icon and does not persist in Action Center properly.
A real client needs its own AUMID registered against a Start Menu shortcut at install time.
Since FastCord is Windows-first, depending on `tauri-winrt-notification` directly is the cleaner choice: it is the same code `notify-rust` would call, with the full toast surface (hero image, icon crop, sound, duration) and no Linux dependency tree.

### Tray icon

`tray-icon` 0.25.1 states that on Windows an event loop must be running on the thread that owns the icon.
eframe runs a winit event loop, so the icon must be created from inside the running app (for example on the first `update`, or via the winit `ActiveEventLoop`), not before `eframe::run_native`.
Tray events arrive on a global channel (`TrayIconEvent::receiver`) rather than through egui's input, so the app has to poll it each frame and call `ctx.request_repaint()`, and the close-to-tray decision means the window must be hidden via the viewport command rather than allowing the close event to terminate the loop.

### Credential storage

`keyring` 4.x is a different shape from the 3.x most documentation describes.
The API and the credential stores were split out: `keyring` now depends on `keyring-core` 1.x and pulls per-platform store crates as optional dependencies.
The default `v1` feature re-creates the classic 3.x-compatible API and, on Windows, enables `windows-native-keyring-store` 1.x, which is the Windows Credential Manager backend.
So `keyring = "4"` with default features gives the expected `Entry::new(service, user).set_password(...)` behaviour on Windows Credential Manager.
Upstream advises applications that want fine-grained control to depend on `keyring-core` plus the specific store instead, and warns against the `cli` feature, which drags in every platform's store.

Worth flagging for the token-storage ticket: Credential Manager blobs are protected per Windows user account, not per application, so any process running as that user can read the token back. That is the same guarantee the official Discord client has, but it should be an explicit accepted risk rather than an assumption.

### Theme

No third-party crate is needed. winit reports the OS theme to egui as `RawInput::system_theme`, `ThemePreference::System` is the default, and egui 0.36.0 added syncing of the OS window decorations to the app theme.
`dark-light` 3.0 (with `detect`, `subscribe` and an async `stream`) is only worth adding if we need the system preference somewhere outside the winit window — and on Windows it would duplicate what winit already delivers.

## Recommended dependency set

```toml
egui = "0.36.2"
eframe = "0.36.2"
egui_extras = { version = "0.36.2", features = ["image", "gif", "webp", "http"] }
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "webp"] }
arboard = "3.6"
tauri-winrt-notification = "0.8"
tray-icon = "0.25"
keyring = "4.2"
```

`egui_commonmark` is deliberately left out of the baseline: Discord's markup is not CommonMark, and pulling it in for the message list would mean fighting its parser. It is a reasonable pick for rendering static in-app help or changelog text.

## Sources

- egui crate metadata and versions — https://crates.io/api/v1/crates/egui
- egui CHANGELOG (0.34–0.36.2) — https://github.com/emilk/egui/blob/main/CHANGELOG.md
- egui README, "Disadvantages of immediate mode" and CPU usage — https://github.com/emilk/egui/blob/main/README.md
- Animated image frame selection, `FrameDurations`, `decode_animated_image_uri` — https://github.com/emilk/egui/blob/main/crates/egui/src/widgets/image.rs
- GIF loader, eager full-animation decode — https://github.com/emilk/egui/blob/main/crates/egui_extras/src/loaders/gif_loader.rs
- egui_extras image loader and format gating — https://github.com/emilk/egui/blob/main/crates/egui_extras/src/loaders/image_loader.rs
- egui_extras CHANGELOG (GIF support #4620, animated WebP #5470) — https://github.com/emilk/egui/blob/main/crates/egui_extras/CHANGELOG.md
- `ScrollArea::show_rows` / `show_viewport` — https://github.com/emilk/egui/blob/main/crates/egui/src/containers/scroll_area.rs
- `Popup` API — https://github.com/emilk/egui/blob/main/crates/egui/src/containers/popup.rs
- Unreleased `CompletionPopup` / `Suggestion` — https://github.com/emilk/egui/blob/main/crates/egui/src/widgets/text_edit/completion.rs
- Unreleased `Event::PasteImage` — https://github.com/emilk/egui/blob/main/crates/egui/src/data/input/event.rs
- Released 0.36.2 `Event` enum (no `PasteImage`) — https://docs.rs/egui/0.36.2/egui/enum.Event.html
- `RawInput::system_theme`, `max_texture_side` — https://github.com/emilk/egui/blob/main/crates/egui/src/data/input/raw_input.rs
- winit theme plumbed into egui — https://github.com/emilk/egui/blob/main/crates/egui-winit/src/lib.rs
- `ThemePreference` — https://github.com/emilk/egui/blob/main/crates/egui/src/memory/theme.rs
- Galley cache and per-frame eviction — https://github.com/emilk/egui/blob/main/crates/epaint/src/text/galley_cache.rs
- egui_extras API docs — https://docs.rs/egui_extras/0.36.2/egui_extras/
- egui_commonmark README and feature list — https://github.com/lampsitter/egui_commonmark/blob/master/README.md, https://crates.io/api/v1/crates/egui_commonmark/0.25.0
- notify-rust README, Windows support — https://github.com/hoodie/notify-rust/blob/main/README.md
- tauri-winrt-notification README, platform limits and AppUserModelID — https://github.com/tauri-apps/winrt-notification/blob/dev/README.md
- tray-icon README, event loop requirement — https://github.com/tauri-apps/tray-icon/blob/dev/README.md
- keyring-rs README, v4 restructuring — https://github.com/open-source-cooperative/keyring-rs/blob/main/README.md
- keyring 4.2.0 features and per-target store dependencies — https://crates.io/api/v1/crates/keyring/4.2.0
- dark-light README — https://github.com/rust-dark-light/rust-dark-light/blob/master/README.md
- arboard `Clipboard::get_image` / `set_image` — https://docs.rs/arboard/3.6.1/arboard/struct.Clipboard.html
