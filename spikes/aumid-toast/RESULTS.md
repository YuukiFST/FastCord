# Spike: registry-only AUMID toast delivery (FastCord #33)

Machine: Windows 10 Pro 19045, non-admin user, two monitors (primary 1600x900).
Crate: tauri-winrt-notification 0.8.1 (windows 0.62), windows-registry 0.6. Portable exe run from a plain folder, no Start Menu shortcut, no installer.
Date: 2026-09-22.

## Setup

`HKCU\Software\Classes\AppUserModelId\FastCord.Client` with `DisplayName=FastCord`, `IconBackgroundColor=0`, `IconUri=<absolute path to icon.png>`.
Toast built with `Toast::new("FastCord.Client")`, one button (`action=open&channel=123`), `on_activated` and `on_dismissed` handlers; process kept alive 45 s; then `ToastNotificationManager::History().GetHistoryWithId("FastCord.Client")`.

## Results

| Run | Setting() | Displayed | Name/icon | Event | History after |
|---|---|---|---|---|---|
| registered, toasts disabled for user (machine default) | DisabledForUser (2) | no | - | none | 0 |
| registered, ToastEnabled=1 without service restart | DisabledForUser (2) | no | - | none | 0 |
| registered, after WpnUserService restart | Enabled (0) | yes | "FastCord" + icon from IconUri | ACTIVATED args=Some("action=open&channel=123") (button clicked) | 0 (activation removes it) |
| registered, second run | Enabled (0) | yes | same | DISMISSED reason=UserCanceled (X clicked) | 1 |
| unregistered (key deleted) | Enabled (0) | not observed on screen | - | none in 45 s | 2 (banked in Action Center anyway) |

Screenshot of the registered run: `toast-registered.png` (toast shows "FastCord", circular icon, title, body, button).

## Facts

1. Registry-only AUMID is enough on 19045: display name, icon, Action Center persistence and click activation with arguments all work with no Start Menu shortcut, as long as the process is alive to receive `Activated`.
2. `show()` returns `Ok(())` even when the toast is suppressed (`DisabledForUser`). Only `ToastNotifier.Setting()` tells the truth.
3. `HKCU\...\PushNotifications\ToastEnabled` is read once per WpnUserService lifetime; changing it needs a service restart or re-logon before `Setting()` changes.
4. The reference machine has toasts disabled for the user (`ToastEnabled=0`) as its normal state.
5. Icon colour on the toast did not match the PNG (blue PNG rendered olive); treat IconUri icons as tinted and use a monochrome silhouette or verify with the final asset.
