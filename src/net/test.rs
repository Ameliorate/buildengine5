use std::sync::atomic::AtomicUsize;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

lazy_static! {
    [pub] static ref TEST_INT: AtomicUsize = AtomicUsize::new(0);
}

#[test]
fn init_event_loop() {
    EventLoop::new().unwrap();
}

#[test]
fn init_handler_listener() {
    let listener = TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), ::net::STANDARD_PORT))).unwrap();
    Handler::new_listener(listener);
}

#[test]
fn init_handler_no_listener() {
    let handler = Handler::new();
}

#[test]
fn init_event_loop_listener() {
    EventLoop::new().unwrap();
}

#[test]
fn init_client() {
    let event_loop = EventLoop::new().unwrap();
    let handler = Handler::new();
    Client::spawn_client(server_address, &event_loop).unwrap();
}
