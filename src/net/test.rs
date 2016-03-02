use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;
use std::io;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::sync::atomic::{AtomicUsize, Ordering};

use byteorder::{ByteOrder, LittleEndian};
use mio::tcp::{TcpListener, TcpStream};

use net::EventLoop;

/// The amount of time to wait while unit testing to make sure threads are syncronized when there is no other method.
const WAIT_TIME_MS: usize = 250;

lazy_static! {
    /// Used for unit testing sends.
    #[derive(Debug)]
    pub static ref TEST_VAL: AtomicUsize = AtomicUsize::new(0);
}

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
    thread::sleep(Duration::from_millis(WAIT_TIME_MS));
    // This may, once in a very long time, fail. It really shouldn't, but it is possible.
    // Just raise the number or try again if it fails.
    match rx.try_recv() {
        Err(TryRecvError::Empty) => {
            panic!("EventLoopImpl did not stop after calling shutdown()! This fact is depended on \
                    by other unit tests, so ctrl+c here")
        }
        Err(TryRecvError::Disconnected) => {
            panic!("EventLoop somehow disconnected it's channel without stopping! This fact is \
                    depended on by other unit tests, so ctrl+c here")
        }
        Ok(res) => res,
    }
    .unwrap();
    thread.join().unwrap();
}

fn event_loop_helper() -> (super::EventLoopImplRef, JoinHandle<super::EventLoopImpl>) {
    let mut event_loop = super::EventLoopImpl::new(super::MAX_CONNECTIONS).unwrap();
    let event_loop_ref: super::EventLoopImplRef = (&mut event_loop).into();
    (event_loop_ref,
     thread::spawn(move || {
        event_loop.run().unwrap();
        event_loop
    }))
}

#[test]
fn event_loop_impl_add_listener() {
    let (event_loop_ref, thread) = event_loop_helper();
    let listener = TcpListener::bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                     0,
                                                                                     0,
                                                                                     1),
                                                                       0)))
                       .unwrap();
    event_loop_ref.add_listener(listener);
    event_loop_ref.shutdown();
    let event_loop = thread.join().unwrap();
    assert_eq!(event_loop.handler.listeners.len(), 1);
}

#[test]
fn event_loop_impl_add_socket() {
    let (event_loop_ref, thread) = event_loop_helper();
    let listener = TcpListener::bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                     0,
                                                                                     0,
                                                                                     1),
                                                                       25567)))
                       .unwrap();
    let _stream_local = TcpStream::connect(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                           0,
                                                                                           0,
                                                                                           1),
                                                                             25567)));
    let stream_remote;
    loop {
        match listener.accept().unwrap() {
            None => {}  // I think this is that there is no socket avalable to accept.
            Some((stream, _addr)) => {
                stream_remote = stream;
                break;
            }
        }
    }
    event_loop_ref.add_socket(stream_remote);
    event_loop_ref.shutdown();
    let event_loop = thread.join().unwrap();
    assert_eq!(event_loop.handler.connections.count(), 1);
}

#[test]
fn event_loop_impl_kill() {
    let (event_loop_ref, thread) = event_loop_helper();
    let listener = TcpListener::bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                     0,
                                                                                     0,
                                                                                     1),
                                                                       25569)))
                       .unwrap();
    let stream_local = TcpStream::connect(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                          0,
                                                                                          0,
                                                                                          1),
                                                                            25569)))
                           .unwrap();
    let _stream_remote;
    loop {
        match listener.accept().unwrap() {
            None => {}  // I think this is that there is no socket avalable to accept.
            Some((stream, _addr)) => {
                _stream_remote = stream;
                break;
            }
        }
    }
    let token = event_loop_ref.add_socket(stream_local);
    event_loop_ref.kill(token);
    event_loop_ref.shutdown();
    let event_loop = thread.join().unwrap();
    assert_eq!(event_loop.handler.connections.count(), 0);
    // TODO: Check if it actually closes the socket.
    // I can't think of a way to do this. It just isn't exposed in mio's API, io::Read's API, nowhere.
    // The closest I can get is reading 0 bytes, but even that is ambiguous.
}

#[test]
fn event_loop_impl_send() {
    let (event_loop_ref_local, thread_local) = event_loop_helper();
    let (event_loop_ref_remote, thread_remote) = event_loop_helper();
    let listener = TcpListener::bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                     0,
                                                                                     0,
                                                                                     1),
                                                                       25568)))
                       .unwrap();
    let stream_local = TcpStream::connect(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                          0,
                                                                                          0,
                                                                                          1),
                                                                            25568)))
                           .unwrap();
    let stream_remote;
    loop {
        match listener.accept().unwrap() {
            None => {}  // I think this is that there is no socket avalable to accept.
            Some((stream, _addr)) => {
                stream_remote = stream;
                break;
            }
        }
    }
    let _token_remote = event_loop_ref_local.add_socket(stream_local);
    let token_local = event_loop_ref_remote.add_socket(stream_remote);

    let old_test_val = TEST_VAL.load(Ordering::Relaxed);
    event_loop_ref_remote.send(token_local, super::NetworkPacket::Test);
    thread::sleep(Duration::from_millis(WAIT_TIME_MS));
    let new_test_val = TEST_VAL.load(Ordering::Relaxed);

    event_loop_ref_local.shutdown();
    event_loop_ref_remote.shutdown();
    let _event_loop_local = thread_local.join().unwrap();
    let _event_loop_remote = thread_remote.join().unwrap();

    assert!(old_test_val < new_test_val,
            "old_test_val < new_test_val, old_test_val: {}, new_test_val: {}",
            old_test_val,
            new_test_val);
}
