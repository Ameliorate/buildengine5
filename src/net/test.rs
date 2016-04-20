use mio::tcp::TcpListener;
use mioco::MioAdapter;

use super::*;
use test_util;
use test_util::Tattle;

#[test]
fn nethandle_new() {
    test_util::start_log_once();
    let tattle = Tattle::new();
    let listener = MioAdapter::new(TcpListener::bind(&ip("0.0.0.0:0")).unwrap());
    assert!(tattle.changed(|| {
        NetHandle::new_tattle_server(Some(tattle.clone()), None, listener);
    }));
}

#[test]
fn nethandle_shutdown() {
    test_util::start_log_once();
    let tattle = Tattle::new();
    let listener = MioAdapter::new(TcpListener::bind(&ip("0.0.0.0:0")).unwrap());
    let h = NetHandle::new_tattle_server(None, Some(tattle.clone()), listener);
    assert!(tattle.changed(|| {
        h.shutdown().unwrap();
    }))
}
