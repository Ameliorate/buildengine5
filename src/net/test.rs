use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use byteorder::{ByteOrder, LittleEndian};
use mio::tcp::{TcpListener, TcpStream};

use net::EventLoop;

/// The amount of time to wait while unit testing to make sure threads are syncronized when there is no other method.
const WAIT_TIME_MS: u64 = 250;

pub static CLIENT_SERVER_SEND_TEST_VAL: AtomicUsize = AtomicUsize::new(0);

pub static EVENT_LOOP_SEND_TEST_VAL: AtomicUsize = AtomicUsize::new(0);

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum TestValToModify {
    ClientServerSend,
    EventLoopImplSend,
}

/// Tests a client connecting to a server, going through the event loop.
#[test]
fn client_server_connect() {
    let (mut event_loop_ref_client, thread_client) = event_loop_helper();
    let (event_loop_ref_server, thread_server) = event_loop_helper();
    let listener = TcpListener::bind(&super::ip("127.0.0.1:25570")).unwrap();
    event_loop_ref_server.add_listener(listener);
    let client = super::client::Client::spawn_client(super::ip("127.0.0.1:25570"),
                                                     &event_loop_ref_client)
                     .unwrap();
    client.shutdown(&mut event_loop_ref_client);
    event_loop_ref_client.shutdown();
    event_loop_ref_server.shutdown();
    let _event_loop_client = thread_client.join().unwrap();
    let _event_loop_server = thread_server.join().unwrap();
}

/// Tests the event loop sending and receving, using the client infrastructure.
#[test]
fn client_server_send() {
    let (mut event_loop_ref_client, thread_client) = event_loop_helper();
    let (event_loop_ref_server, thread_server) = event_loop_helper();
    let listener = TcpListener::bind(&super::ip("127.0.0.1:25571")).unwrap();
    event_loop_ref_server.add_listener(listener);
    let client = super::client::Client::spawn_client(super::ip("127.0.0.1:25571"),
                                                     &event_loop_ref_client)
                     .unwrap();
    let old_test_val = CLIENT_SERVER_SEND_TEST_VAL.load(Ordering::Relaxed);

    client.send(&event_loop_ref_client,
                super::NetworkPacket::Test(TestValToModify::ClientServerSend));
    thread::sleep(Duration::from_millis(WAIT_TIME_MS));

    client.shutdown(&mut event_loop_ref_client);
    event_loop_ref_client.shutdown();
    event_loop_ref_server.shutdown();
    let _event_loop_client = thread_client.join().unwrap();
    let _event_loop_server = thread_server.join().unwrap();
    let new_test_val = CLIENT_SERVER_SEND_TEST_VAL.load(Ordering::Relaxed);
    assert!(old_test_val < new_test_val,
            "old_test_val < new_test_val, old_test_val: {}, new_test_val: {}",
            old_test_val,
            new_test_val);
}

/// Adds a listener to EventLoopImpl.
#[test]
fn event_loop_impl_add_listener() {
    let (event_loop_ref, thread) = event_loop_helper();
    let listener = TcpListener::bind(&super::ip("127.0.0.1:0")).unwrap();
    event_loop_ref.add_listener(listener);
    event_loop_ref.shutdown();
    let event_loop = thread.join().unwrap();
    assert_eq!(event_loop.handler.listeners.len(), 1);
}

/// Shuts down a EventLoopImpl.
///
/// If this test fails, all other tests will hang, due to the fact that all other tests block on shutting down the event loop.
#[test]
fn event_loop_impl_add_socket() {
    let (event_loop_ref, thread) = event_loop_helper();
    let listener = TcpListener::bind(&super::ip("127.0.0.1:25567")).unwrap();
    let _stream_local = TcpStream::connect(&super::ip("127.0.0.1:25567")).unwrap();
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

/// Constructs an event loop on a new thread.
fn event_loop_helper() -> (super::EventLoopImplRef, JoinHandle<super::EventLoopImpl>) {
    let mut event_loop = super::EventLoopImpl::new(super::MAX_CONNECTIONS).unwrap();
    let event_loop_ref: super::EventLoopImplRef = (&mut event_loop).into();
    (event_loop_ref,
     thread::spawn(move || {
        event_loop.run().unwrap();
        event_loop
    }))
}

/// Kills a connection on an EventLoopImpl.
#[test]
fn event_loop_impl_kill() {
    let (event_loop_ref, thread) = event_loop_helper();
    let listener = TcpListener::bind(&super::ip("127.0.0.1:25568")).unwrap();
    let stream_local = TcpStream::connect(&super::ip("127.0.0.1:25568")).unwrap();
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

/// Constructs an EventLoopImpl.
#[test]
fn event_loop_impl_new() {
    super::EventLoopImpl::new(super::MAX_CONNECTIONS).unwrap();
}

/// Sends a packet using the event loop, but without the client helper struct.
#[test]
fn event_loop_impl_send() {
    let (event_loop_ref_local, thread_local) = event_loop_helper();
    let (event_loop_ref_remote, thread_remote) = event_loop_helper();
    let listener = TcpListener::bind(&super::ip("127.0.0.1:25569")).unwrap();
    let stream_local = TcpStream::connect(&super::ip("127.0.0.1:25569")).unwrap();
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

    let old_test_val = EVENT_LOOP_SEND_TEST_VAL.load(Ordering::Relaxed);
    event_loop_ref_remote.send(token_local,
                               super::NetworkPacket::Test(TestValToModify::EventLoopImplSend));
    thread::sleep(Duration::from_millis(WAIT_TIME_MS));

    event_loop_ref_local.shutdown();
    event_loop_ref_remote.shutdown();
    let _event_loop_local = thread_local.join().unwrap();
    let _event_loop_remote = thread_remote.join().unwrap();
    let new_test_val = EVENT_LOOP_SEND_TEST_VAL.load(Ordering::Relaxed);
    assert!(old_test_val < new_test_val,
            "old_test_val < new_test_val, old_test_val: {}, new_test_val: {}",
            old_test_val,
            new_test_val);
}

/// Shuts down an EventLoopImpl.
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

/// Tests get_packet_length with a bad magic number.
#[test]
fn get_packet_length_bad_magic_number() {
    let length: Option<u16> = super::get_packet_length([0xFE, 0xF0, 0xF6, 0xFD, 0, 10]);
    assert_eq!(length, None);
}

/// Tests get_packet_length with a correct magic number, and ensures the length is the same.
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

/// Constructs a Handler.
#[test]
fn handler_new() {
    super::Handler::new(super::MAX_CONNECTIONS);
}
