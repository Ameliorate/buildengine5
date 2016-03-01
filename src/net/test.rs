use std::thread;
use std::sync::mpsc::{TryRecvError, channel};
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
    let length = super::get_packet_length([m_number[0],
                                           m_number[1],
                                           m_number[2],
                                           m_number[3],
                                           intended_length[0],
                                           intended_length[1]])
                     .expect("get_packet_length wrongly checks NET_MAGIC_NUMBER");
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
}

#[test]
fn event_loop_impl_shutdown() {
    let (tx, rx) = channel::<Result<(), io::Error>>();
    let mut event_loop = super::EventLoopImpl::new(super::MAX_CONNECTIONS).unwrap();
    let event_loop_ref: super::EventLoopImplRef = (&mut event_loop).into();
    let thread = thread::spawn(move || tx.send(event_loop.run()).unwrap());
    event_loop_ref.shutdown();
    thread::sleep(Duration::from_millis(250));
    // This may, once in a very long time, fail. It really shouldn't, but it is possible.
    // Just raise the number or try again if it fails.
    match rx.try_recv() {
        Err(TryRecvError::Empty) => panic!("EventLoopImpl did not stop after calling shutdown()"),
        Err(TryRecvError::Disconnected) => {
            panic!("EventLoop somehow disconnected it's channel without stopping")
        }
        Ok(res) => res,
    }
    .unwrap();
    thread.join().unwrap();
}
