use std::thread;
use std::sync::mpsc::{TryRecvError, channel};
use std::sync::Arc;
use std::time::Duration;
use std::io;

use byteorder::{ByteOrder, LittleEndian};

use net::EventLoop;

#[test]
fn get_packet_length_correct() {
    let mut m_number: [u8; 4] = [0; 4];
    LittleEndian::write_u32(&mut m_number, super::NET_MAGIC_NUMBER);
    let mut intended_length: [u8; 2] = [0; 2];
    LittleEndian::write_u16(&mut intended_length, 10);
    let length = super::get_packet_length([m_number[0], m_number[1], m_number[2], m_number[3], intended_length[0], intended_length[1]]).expect("get_packet_length wrongly checks NET_MAGIC_NUMBER");
    assert_eq!(length, 10);
}

#[test]
fn get_packet_length_bad_magic_number() {
    let length: Option<u16> = super::get_packet_length([0xFE, 0xF0, 0xF6, 0xFD, 0, 10]);
    assert_eq!(length, None);
}

#[test]
fn handler_new() {
    super::Handler::new(super::MAX_CONNECTIONS);
}

#[test]
fn event_loop_impl_new() {
    super::EventLoopImpl::new(super::MAX_CONNECTIONS).unwrap();
    unimplemented!()
}

#[test]
fn event_loop_impl_shutdown() {}
