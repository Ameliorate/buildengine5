//! Contains code relating to networking.

#[cfg(test)]
mod test;

use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::mpsc::SendError;

use bincode::serde::{DeserializeError, deserialize, serialize};
use bincode::SizeLimit;
use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use mioco;
use mioco::sync::mpsc::{Sender, channel};

use test_util::Tattle;

/// Standard number to ensure network connections are syncronized and the same protocol is being used.
/// Reexported incase it is of use for something not-networking.
pub const NET_MAGIC_NUMBER: u32 = 0xCB011043; //0xcafebade + 0x25565, because programming references.

/// Represents the network state and provides various utilities acting upon it.
#[allow(missing_debug_implementations)]
pub struct NetHandle(Sender<()>);

impl NetHandle {
    /// Construct a new instance, starting a new coroutine and opening all network traffic on the spesified port.
    pub fn new() -> Self {
        NetHandle::new_tattle(None, None)
    }

    /// Constructs, with several optional structs that use global state for easier unit testing.
    pub fn new_tattle(tattle_closure_start: Option<Tattle>,
                      tattle_shutdown: Option<Tattle>)
                      -> Self {
        let (tx, rx) = channel::<()>(); // TODO: Maybe have more possible messages than () to shutdown?
        mioco::spawn(move || {
            if let Some(tattle) = tattle_closure_start {
                tattle.call();
            }
            loop {
                select!(
                    rx:r => {
                        if let Some(tattle) = tattle_shutdown {
                            tattle.call();
                        }
                        debug!("Shutting down coroutine");
                        let _ = rx.recv();
                        break;
                    },
                );
            }
        });
        NetHandle(tx)
    }

    /// Shuts down the socket/listener, closing all connections.
    pub fn shutdown(&self) -> Result<(), SendError<()>> {
        self.0.send(())
    }
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
    /// Increments a value internally. It's supposed to be used for unit testing.
    #[cfg(test)]
    Test(test::TestValToModify),
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
        panic!("Because localhost can resolve to both 127.0.0.1, and the vairous IPV6 versions \
                of 127.0.0.1, it may not be used. Please instead use 127.0.0.1");
    }
    let mut iter = ip_addr.to_socket_addrs().unwrap();
    let ip = iter.next().unwrap();
    if iter.next() != None {
        panic!("The given ip to net::ip() resolved to more than 1 SocketAddr");
    }
    ip
}

fn deserialize_packet(to_de: &[u8]) -> Result<NetworkPacket, DeserializeError> {
    deserialize(to_de)
}

/// Returns the length of a given packet, or a None if the first four bytes do not match NET_MAGIC_NUMBER.
fn get_packet_length(to_ln: [u8; 6]) -> Option<u16> {
    let (first_four, next_two) = to_ln.split_at(4);
    let should_be_magic_num = LittleEndian::read_u32(&first_four);
    if should_be_magic_num != NET_MAGIC_NUMBER {
        return None;
    }
    let length = LittleEndian::read_u16(&next_two);
    Some(length)
}

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
