use super::*;

fn claim(kind: ServerKind, root: &Path) -> ServerIdentity {
    ServerIdentity::claim_in(kind, Some(root.to_owned()))
}

fn name(identity: &ServerIdentity) -> String {
    identity.name().expect("claimed a name")
}

/// **The background server is `default`, and only one can be** — a second says why it has no
/// folder instead of sharing the first one's.
#[test]
fn the_background_server_is_default_and_a_second_one_has_no_folder() {
    let root = tempfile::tempdir().unwrap();
    let first = claim(ServerKind::Background, root.path());
    assert_eq!(name(&first), "default");
    let second = claim(ServerKind::Background, root.path());
    assert_eq!(second.name(), None);
    let error = second.service_folder("demo.one").unwrap_err();
    assert!(error.to_string().contains("already running"), "{error}");
    drop(first);
    assert_eq!(name(&claim(ServerKind::Background, root.path())), "default", "free again once the first is gone");
}

/// **Standalone servers take the lowest free slot, and a freed slot is the next one taken.**
#[test]
fn standalone_servers_take_the_lowest_free_slot() {
    let root = tempfile::tempdir().unwrap();
    let first = claim(ServerKind::Standalone, root.path());
    let second = claim(ServerKind::Standalone, root.path());
    let third = claim(ServerKind::Standalone, root.path());
    assert_eq!([name(&first), name(&second), name(&third)], ["standalone-1", "standalone-2", "standalone-3"]);
    drop(second);
    assert_eq!(name(&claim(ServerKind::Standalone, root.path())), "standalone-2");
    drop(first);
    assert_eq!(name(&claim(ServerKind::Standalone, root.path())), "standalone-1");
}

/// **The background server and the standalone ones never take each other's name.**
#[test]
fn the_kinds_do_not_collide() {
    let root = tempfile::tempdir().unwrap();
    let background = claim(ServerKind::Background, root.path());
    let standalone = claim(ServerKind::Standalone, root.path());
    assert_eq!((name(&background), name(&standalone)), ("default".into(), "standalone-1".into()));
}

/// **A service's folder is made owner-only, once per service, and is the same path every time.**
#[test]
fn a_service_folder_is_private_made_on_asking_and_stable() {
    let root = tempfile::tempdir().unwrap();
    let identity = claim(ServerKind::Background, root.path());
    let one = identity.service_folder("demo.one").unwrap();
    assert_eq!(one, root.path().join("default").join("demo.one"));
    assert!(one.is_dir());
    assert_ne!(one, identity.service_folder("demo.two").unwrap());
    assert_eq!(identity.service_folder("demo.one").unwrap(), one);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&one, std::fs::Permissions::from_mode(0o755)).unwrap();
        identity.service_folder("demo.one").unwrap();
        for folder in [&one, one.parent().unwrap(), root.path()] {
            assert_eq!(std::fs::metadata(folder).unwrap().permissions().mode() & 0o777, 0o700, "{folder:?}");
        }
    }
}

/// **A service name that is a path is refused**, so a folder can never be made outside the server's.
#[test]
fn a_service_name_that_is_a_path_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let identity = claim(ServerKind::Background, root.path());
    for bad in ["", "..", ".", "../escape", "a/b", "/etc"] {
        assert!(identity.service_folder(bad).is_err(), "{bad:?}");
    }
    assert!(!root.path().join("escape").exists());
}

/// **A platform with no data folder gives an identity with no folder** — a reason, not a panic.
#[test]
fn no_data_folder_is_a_reason_not_a_panic() {
    let identity = ServerIdentity::claim_in(ServerKind::Standalone, None);
    let error = identity.service_folder("demo.one").unwrap_err();
    assert!(error.to_string().contains("no data folder"), "{error}");
}
