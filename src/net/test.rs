use super::*;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

#[test]
fn ip_correct() {
    let ip = ip("8.8.8.8:80");
    let correct = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(8, 8, 8, 8), 80));
    assert_eq!(ip, correct);
}

#[test]
#[should_panic(expected = "because localhost can resolve to both 127.0.0.1, and the various IPV6 versions \
        of 127.0.0.1, it may not be used. please instead use 127.0.0.1")]
fn ip_localhost() {
    ip("localhost:80");
}
