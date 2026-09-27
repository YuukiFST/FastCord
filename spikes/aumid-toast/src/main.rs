//! Spike for FastCord ticket #33: does a toast from an unpackaged, portable exe
//! whose AUMID is registered only under HKCU\Software\Classes\AppUserModelId
//! display with name + icon, persist in Action Center, and deliver click activation?
//!
//! usage: aumid-toast-spike [register|unregistered|cleanup]
use std::{path::absolute, sync::mpsc, time::Duration};
use tauri_winrt_notification::{Scenario, Toast, ToastDismissalReason};
use windows::{core::HSTRING, UI::Notifications::ToastNotificationManager};
use windows_registry::CURRENT_USER;

const APP_ID: &str = "FastCord.Client";
const APP_NAME: &str = "FastCord";
const WAIT_SECS: u64 = 45;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "register".into());
    let key = format!(r"SOFTWARE\Classes\AppUserModelId\{APP_ID}");
    match mode.as_str() {
        "cleanup" => {
            CURRENT_USER.remove_tree(&key).expect("remove_tree");
            println!("registry cleaned");
            return;
        }
        "register" => {
            let icon = absolute("icon.png").unwrap();
            let k = CURRENT_USER.create(&key).expect("create key");
            k.set_string("DisplayName", APP_NAME).unwrap();
            k.set_string("IconBackgroundColor", "0").unwrap();
            k.set_hstring("IconUri", &icon.as_path().into()).unwrap();
            println!("registered {APP_ID} -> {APP_NAME}, icon {}", icon.display());
        }
        "unregistered" => {
            let _ = CURRENT_USER.remove_tree(&key);
            println!("running with NO registry entry for {APP_ID}");
        }
        other => panic!("unknown mode {other}"),
    }

    match ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID)).and_then(|n| n.Setting()) {
        Ok(setting) => println!("NotificationSetting for {APP_ID}: {setting:?} (0=Enabled 1=DisabledForApplication 2=DisabledForUser 3=DisabledByGroupPolicy 4=DisabledByManifest)"),
        Err(e) => println!("Setting() failed: {e}"),
    }

    let (tx, rx) = mpsc::channel::<String>();
    let tx_a = tx.clone();
    let tx_d = tx.clone();
    let shown = Toast::new(APP_ID)
        .title("FastCord spike")
        .text1("Click me, or the button, or dismiss me.")
        .add_button("Open channel", "action=open&channel=123")
        .scenario(Scenario::Default)
        .on_activated(move |arg| {
            tx_a.send(format!("ACTIVATED args={arg:?}")).ok();
            Ok(())
        })
        .on_dismissed(move |reason| {
            let r = match reason {
                Some(ToastDismissalReason::UserCanceled) => "UserCanceled",
                Some(ToastDismissalReason::ApplicationHidden) => "ApplicationHidden",
                Some(ToastDismissalReason::TimedOut) => "TimedOut",
                _ => "Unknown",
            };
            tx_d.send(format!("DISMISSED reason={r}")).ok();
            Ok(())
        })
        .show();
    println!("show() -> {shown:?}");

    println!("waiting {WAIT_SECS}s for activation/dismissal events...");
    let deadline = std::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(msg) => println!("event: {msg}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }

    let history = ToastNotificationManager::History().and_then(|h| h.GetHistoryWithId(&HSTRING::from(APP_ID)));
    match history {
        Ok(v) => println!("action-center history for {APP_ID}: {} toast(s)", v.Size().unwrap_or(0)),
        Err(e) => println!("history query failed: {e}"),
    }
}
