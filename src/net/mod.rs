//! Contains code relating to networking.

#[cfg(test)]
mod test;

use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::mpsc::{SendError, TryRecvError};

use bincode::serde::{DeserializeError, deserialize, serialize};
use bincode::SizeLimit;
use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use mioco;
use mioco::sync::mpsc::{Receiver, Sender, channel};
use mioco::tcp::{TcpListener, TcpStream};
use slab::Slab;

use test_util::Tattle;

/// Standard number to ensure network connections are syncronized and the same protocol is being used.
///
/// Reexported incase it is of use for something not-networking.
pub const NET_MAGIC_NUMBER: u32 = 0xCB011043; //0xcafebade + 0x25565, because programming references.

/// The maximum number of clients allowed to be connected at one time.
///
/// Eventually this should be removed and replaced with something configurable.
pub const MAX_CONNECTED_CLIENTS: usize = 30;

/// Represents the network state and provides various utilities acting upon it.
#[allow(missing_debug_implementations)]
pub struct NetHandle(Sender<NetAction>);

impl NetHandle {
    /// Construct a new instance, starting a new coroutine and opening all network traffic on the spesified port.
    pub fn new_server(listener: TcpListener) -> Self {
        NetHandle::new_tattle_server(None, None, listener)
    }

    /// Constructs, with several optional structs that use global state for easier unit testing.
    pub fn new_tattle_server(tattle_closure_start: Option<Tattle>,
                             tattle_shutdown: Option<Tattle>,
                             listener: TcpListener)
                             -> Self {
        let (tx, rx) = channel::<NetAction>();
        let mut clients: Slab<Sender<NetAction>, usize> = Slab::new(MAX_CONNECTED_CLIENTS);
        mioco::spawn(move || {
            if let Some(tattle) = tattle_closure_start {
                tattle.call();
            }
            loop {
                select!(
                    rx:r => {
                        use net::NetAction::*;
                        match rx.recv().expect("channel to net coroutine improperly closed") {
                            Shutdown => {
                                if let Some(tattle) = tattle_shutdown {
                                    tattle.call();
                                }
                                debug!("shutting down coroutine");
                                for client_tx in clients.iter() {
                                    let _ = client_tx.send(Shutdown);
                                }
                                break;
                            }
                        }
                    },
                    listener:r => {
                        match listener.try_accept() {
                            Ok(Some(peer)) => {
                                let (client_tx, client_rx) = channel::<NetAction>();
                                let id = match clients.insert(client_tx) {
                                    Ok(id) => id,
                                    Err(_client_tx) => {
                                        info!("Client attempted to connect but the maximum number of connections was reached.");
                                        continue
                                    }
                                };
                                info!("Client {} connected", id);
                                mioco::spawn(move || check_stream(peer, client_rx, id));
                            }
                            Ok(None) => {}
                            Err(err) => panic!("io::Error when accepting connections in server net loop: {}", err),
                        }
                    },
                );
            }
        });
        NetHandle(tx)
    }

    /// Shuts down the socket/listener, closing all connections.
    pub fn shutdown(&self) -> Result<(), SendError<NetAction>> {
        self.0.send(NetAction::Shutdown)
    }
}

/// Represents all the possible actions inside a network coroutine.
///
/// Maps mostly 1:1 with the interface of NetHandle.
#[derive(Clone, Copy, Debug)]
pub enum NetAction {
    /// Kill the coroutine, and any open connections.
    Shutdown,
}

/// Sent in the case of an error that should be sent to the peer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkError {
    /// If the versions mismatch sufficently to become incompatible with each other.
    VersionMismatch(String, String),
    /// If both peers have should_crash == false, then this error should be sent.
    ///
    /// Do note that this error should not be rewrapped into a reerror, since it would cause a loop.
    /// Instead, it should be logged and ignored, as the connection will be killed shortly after.
    ShouldCrashBothTrue,
}

impl Display for NetworkError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            NetworkError::VersionMismatch(ref ver1, ref ver2) => {
                write!(fmt,
                       "VersionMismatch: The versions of the client and server attempting to \
                        connect mismatch. ver1: {}, ver2: {}",
                       ver1,
                       ver2)
            }
            NetworkError::ShouldCrashBothTrue => {
                write!(fmt,
                       "ShouldCrashBothTrue: Both peers have should_crash == false.")
            }
        }
    }
}

impl Error for NetworkError {
    fn description(&self) -> &str {
        match *self {
            NetworkError::VersionMismatch(_, _) => {
                "VersionMismatch: The versions of the client and server attempting to connect \
                 mismatch."
            }
            NetworkError::ShouldCrashBothTrue => {
                "ShouldCrashBothTrue: Both peers have should_crash == false."
            }
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            NetworkError::VersionMismatch(_, _) => None,
            NetworkError::ShouldCrashBothTrue => None,
        }
    }
}

/// Messages that can be sent between peers to facilitate vairous actions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkPacket {
    /// Sent on connection to verify everything is in sync.
    Init {
        /// The curent version of the local game.
        ///
        /// Should be formatted according to Scematic Versioning.
        version: String,
        /// If the local game should crash when an error occours.
        ///
        /// Two peers should not have this as false, because should_crash will send the error to the
        /// remote peer and make it crash instead.
        /// This would cause a infinite loop if both were to do it.
        should_crash: bool,
    },
    /// An error that should crash the game and show an error to the user, but only on a client.
    Error(NetworkError),
}

/// Parses a str to a SocketAddr.
///
/// This is a function because while str implements ToSocketAddrs, it requires a good bit of boilerplate to use.
///
/// #Panics
/// * Calling with a localhost ip address: Use 127.0.0.1 instead.
/// * Calling with an ip address that resolves to more than 1 ip address.
pub fn ip(ip_addr: &str) -> SocketAddr {
    if ip_addr.starts_with("localhost") {
        panic!("because localhost can resolve to both 127.0.0.1, and the various IPV6 versions \
                of 127.0.0.1, it may not be used. please instead use 127.0.0.1");
    }
    let mut iter = ip_addr.to_socket_addrs().unwrap();
    let ip = iter.next().unwrap();
    if iter.next() != None {
        panic!("the given ip to net::ip() resolved to more than 1 SocketAddr");
    }
    ip
}

fn check_stream(mut stream: TcpStream, receiver: Receiver<NetAction>, id: usize) {
    let mut send_queue: Vec<NetworkPacket> = Vec::new();
    loop {
        select!(
            receiver:r => {
                use net::NetAction::*;
                match receiver.try_recv() {
                    Ok(Shutdown) => {
                        info!("Shutting down client at with id {}", id);
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        info!("Shutting down client at with id {}", id);
                        debug!("shutting down client {} due to disconnected channel", id);
                        break;
                    }
                }
            },
            stream:r => {

            },
            stream:w => {
                let send_queue_old = send_queue;
                send_queue = Vec::new();
                for packet in send_queue_old {
                    match send_to_stream(&mut stream, &packet) {
                        Ok(Some(_len)) => {}
                        Ok(None) => send_queue.push(packet),
                        Err(err) => panic!("got io::Error writing packet to stream to client {}. packet: {:?}, err: {}",
                                            id, packet, err),
                    }
                }
            },
        );
    }
}

#[allow(unused)]    // TODO: Remove allow(unused).
fn deserialize_packet(to_de: &[u8]) -> Result<NetworkPacket, DeserializeError> {
    deserialize(to_de)
}

/// Returns the length of a given packet, or a None if the first four bytes do not match NET_MAGIC_NUMBER.
#[allow(unused)]
fn get_packet_length(to_ln: [u8; 6]) -> Option<u16> {
    let (first_four, next_two) = to_ln.split_at(4);
    let should_be_magic_num = LittleEndian::read_u32(&first_four);
    if should_be_magic_num != NET_MAGIC_NUMBER {
        return None;
    }
    let length = LittleEndian::read_u16(&next_two);
    Some(length)
}

fn send_to_stream(stream: &mut TcpStream,
                  packet: &NetworkPacket)
                  -> Result<Option<usize>, io::Error> {
    let ser = seralize_packet(&packet);
    stream.try_write(&ser)
}

#[allow(unused)]
fn seralize_packet(to_ser: &NetworkPacket) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    result.write_u32::<LittleEndian>(NET_MAGIC_NUMBER).unwrap();   // No possible errors here.
    // The NET_MAGIC_NUMBER is used before every packet, so incase the stream is desynced for whatever reason, the game doesn't just read arbratrary data and crash badly.
    // Instead, it can either recover somehow, by disconnecting and reconnecting, or just erroring gracefully.
    let mut encoded = serialize(to_ser, SizeLimit::Infinite).unwrap();
    // Since the size limit is infinite and i'm not encoding to a stream, there is no error and I can safely unwrap();
    result.write_u16::<LittleEndian>(encoded.len() as u16).unwrap();
    result.append(&mut encoded);
    result
}
