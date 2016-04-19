use super::*;
use test_util;
use test_util::Tattle;

#[test]
fn nethandle_new() {
    test_util::start_log_once();
    let tattle = Tattle::new();
    assert!(tattle.changed(|| {
        NetHandle::new_tattle(Some(tattle.clone()), None);
    }));
}

#[test]
fn nethandle_shutdown() {
    test_util::start_log_once();
    let tattle = Tattle::new();
    let h = NetHandle::new_tattle(None, Some(tattle.clone()));
    assert!(tattle.changed(|| {
        h.shutdown().unwrap();
    }))
}
