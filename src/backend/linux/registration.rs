use zbus::blocking::{Connection, Proxy};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";

pub(super) fn refresh(connection: &Connection, registered_owner: &mut Option<String>) {
    let owner = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .and_then(|proxy| proxy.call::<_, _, String>("GetNameOwner", &(WATCHER)));
    let Ok(owner) = owner else {
        *registered_owner = None;
        return;
    };
    if registered_owner.as_ref() == Some(&owner) {
        return;
    }
    let Some(identity) = connection.unique_name() else {
        return;
    };
    // Recovery services discover this connection name too. Using an alias here
    // lets the same item appear twice when recovery races our registration.
    let result = Proxy::new(connection, owner.as_str(), WATCHER_PATH, WATCHER).and_then(|proxy| {
        proxy.call::<_, _, ()>("RegisterStatusNotifierItem", &(identity.as_str()))
    });
    if result.is_ok() {
        *registered_owner = Some(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashSet,
        io::{BufRead, BufReader},
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };
    use zbus::blocking::connection::Builder;

    struct Bus {
        child: Child,
        address: String,
    }

    impl Bus {
        fn new() -> Self {
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("tests require dbus-daemon");
            let mut address = String::new();
            BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut address)
                .unwrap();
            Self {
                child,
                address: address.trim().to_owned(),
            }
        }

        fn connection(&self) -> Connection {
            Builder::address(self.address.as_str())
                .unwrap()
                .build()
                .unwrap()
        }

        fn watcher(&self, state: Arc<WatcherState>) -> Connection {
            Builder::address(self.address.as_str())
                .unwrap()
                .serve_at(WATCHER_PATH, Watcher { state })
                .unwrap()
                .name(WATCHER)
                .unwrap()
                .build()
                .unwrap()
        }
    }

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[derive(Default)]
    struct WatcherState {
        items: Mutex<HashSet<String>>,
        calls: AtomicUsize,
        reject_next: AtomicBool,
    }

    struct Watcher {
        state: Arc<WatcherState>,
    }

    #[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
    impl Watcher {
        fn register_status_notifier_item(&self, service: &str) -> zbus::fdo::Result<()> {
            self.state.calls.fetch_add(1, Ordering::SeqCst);
            if self.state.reject_next.swap(false, Ordering::SeqCst) {
                return Err(zbus::fdo::Error::Failed("watcher not ready".into()));
            }
            self.state
                .items
                .lock()
                .unwrap()
                .insert(format!("{service}/StatusNotifierItem"));
            Ok(())
        }
    }

    #[test]
    fn recovery_and_native_registration_share_one_identity() {
        let bus = Bus::new();
        let state = Arc::new(WatcherState::default());
        let _watcher = bus.watcher(state.clone());
        let app = bus.connection();
        let alias = "org.kde.StatusNotifierItem.Test";
        app.request_name(alias).unwrap();
        let owner = app.unique_name().unwrap().as_str();
        let recovery = bus.connection();
        let proxy = Proxy::new(&recovery, WATCHER, WATCHER_PATH, WATCHER).unwrap();
        for recovery_first in [true, false] {
            state.items.lock().unwrap().clear();
            let mut registered = None;
            if recovery_first {
                proxy
                    .call::<_, _, ()>("RegisterStatusNotifierItem", &(owner))
                    .unwrap();
            }
            refresh(&app, &mut registered);
            if !recovery_first {
                proxy
                    .call::<_, _, ()>("RegisterStatusNotifierItem", &(owner))
                    .unwrap();
            }
            assert_eq!(
                *state.items.lock().unwrap(),
                HashSet::from([format!("{owner}/StatusNotifierItem")])
            );
        }
    }

    #[test]
    fn watcher_replacement_is_detected_without_observing_an_absent_watcher() {
        let bus = Bus::new();
        let app = bus.connection();
        let first = Arc::new(WatcherState::default());
        let watcher = bus.watcher(first.clone());
        let mut registered = None;
        refresh(&app, &mut registered);
        refresh(&app, &mut registered);
        assert_eq!(first.calls.load(Ordering::SeqCst), 1);
        watcher.release_name(WATCHER).unwrap();
        let second = Arc::new(WatcherState::default());
        let _replacement = bus.watcher(second.clone());
        refresh(&app, &mut registered);
        assert_eq!(second.calls.load(Ordering::SeqCst), 1);
        assert_eq!(second.items.lock().unwrap().len(), 1);
    }

    #[test]
    fn missing_watcher_and_failed_registration_are_retried() {
        let bus = Bus::new();
        let app = bus.connection();
        let mut registered = None;
        refresh(&app, &mut registered);
        assert!(registered.is_none());
        let state = Arc::new(WatcherState::default());
        state.reject_next.store(true, Ordering::SeqCst);
        let _watcher = bus.watcher(state.clone());
        refresh(&app, &mut registered);
        assert!(registered.is_none());
        refresh(&app, &mut registered);
        assert!(registered.is_some());
        assert_eq!(state.items.lock().unwrap().len(), 1);
        assert_eq!(state.calls.load(Ordering::SeqCst), 2);
    }
}
