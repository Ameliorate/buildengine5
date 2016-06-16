use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;
use std::thread;

use test_util::{TEST_SLEEP_TIME_MILLIS, Tattle, start_log_once};

#[test]
fn check_controller_channel_runs() {
    start_log_once();
    let (tx, rx): (Sender<super::ControllerMessage>, _) = channel();
    let controller_raw = Arc::new(super::ControllerRaw {
        connections: RwLock::new(Vec::new()),
        tx: Mutex::new(tx),
    });
    let controller_raw_clone = controller_raw.clone();
    thread::spawn(move || super::check_controller_channel(rx, controller_raw_clone));
    let tattle = Tattle::new();
    tattle.assert_changed(|| {
        controller_raw.tx
                      .lock()
                      .unwrap()
                      .send(super::ControllerMessage::Test(tattle.clone()))
                      .unwrap();
        thread::sleep(Duration::from_millis(TEST_SLEEP_TIME_MILLIS));
    });
}

#[test]
fn ip_correct() {
    start_log_once();
    let ip = super::ip("8.8.8.8:80");
    let correct = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(8, 8, 8, 8), 80));
    assert_eq!(ip, correct);
}

#[test]
#[should_panic(expected = "because localhost can resolve to both 127.0.0.1, and the various IPV6 versions \
        of 127.0.0.1, it may not be used. please instead use 127.0.0.1")]
fn ip_localhost() {
    start_log_once();
    super::ip("localhost:80");
}
