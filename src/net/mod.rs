use std::convert::From;
use std::io::{Read, Write};
use std::sync::mpsc::{Sender, channel};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::error::Error;

use VERSION;

use bincode::serde::{DeserializeError, deserialize, serialize};
use bincode::SizeLimit;
use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use mio::{EventLoop as MioEventLoop, EventSet, Handler as MioHandler, PollOpt, Token};
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;
use slab::Index;

pub mod client;
pub mod server;
#[cfg(Test)]
pub mod test;

/// The default port for a server to listen on.
pub const STANDARD_PORT: u16 = 25566;
/// Standard number to ensure network connections are syncronized and the same protocol is being used.
/// Reexported incase it is of use for something not-networking.
pub const NET_MAGIC_NUMBER: u32 = 0xCB011043; //0xcafebade + 0x25565, because programming references.
const MAX_CONNECTIONS: usize = 1024;

pub enum HandlerMessage {
    Send(NetworkPacket, Token),
    AddStream(TcpStream, Sender<Token>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkPacket {
    /// Sent on connection to verify everything is in sync.
    Init {
        version: String,
    },
    /// An error that should crash the game and show an error to the user, but only on a client.
    Error(NetworkError),
    /// Used to unit test networking. When recived, increments a value. TODO: Locate that value.
    #[cfg(test)]
    Test,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkError {
    VersionMismatch(String, String),
}

impl Display for NetworkError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            NetworkError::VersionMismatch(ref ver1, ref ver2) => {
                write!(fmt,
                       "VersionMismatch: The versions of the client and server attempting to connect mismatch. ver1: {}, ver2: {}",
                       ver1,
                       ver2)
            }
        }
    }
}

impl Error for NetworkError {
    fn description(&self) -> &str {
        match *self {
            NetworkError::VersionMismatch(_, _) => "VersionMismatch: The versions of the client and server attempting to connect mismatch.",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            NetworkError::VersionMismatch(_, _) => None,
        }
    }
}

pub type EventLoop = MioEventLoop<Handler>;
pub struct Handler {
    connections: Slab<Connection>,
    listener: Option<TcpListener>,
}

impl Handler {
    /// Creates a new instance of a Handler. Does not listen for connections.
    pub fn new() -> Self {
        Handler {
            connections: Slab::new_starting_at(Token::from_usize(1), MAX_CONNECTIONS),
            listener: None,
        }
    }

    /// Creates a new instance of a Handler. Does listen for connections.
    pub fn new_listener() -> Self {
        unimplemented!();
    }
}

#[derive(Debug)]
struct Connection {
    message_queue: Vec<NetworkPacket>,
    stream: TcpStream,
}

impl Connection {
    fn new(stream: TcpStream) -> Connection {
        Connection {
            stream: stream,
            message_queue: Vec::new(),
        }
    }
}

impl MioHandler for Handler {
    type Timeout = ();
    type Message = HandlerMessage;
    fn ready(&mut self, event_loop: &mut MioEventLoop<Handler>, token: Token, events: EventSet) {
        if events.is_writable() {
            // Get all the messages that should be send to that token, seralize all of them, then send all of them.
            // Afterwards, flush the buffer.
            let to_send = self.connections[token].message_queue.clone();
            self.connections[token].message_queue = Vec::new();
            if !to_send.is_empty() {
                for send in to_send {
                    let bytes = seralize_packet(&send);
                    self.connections[token]
                        .stream
                        .write(&bytes)
                        .expect(&format!("Error writing {:?} to connection {:?}/{:?}",
                                         send,
                                         token,
                                         self.connections[token]));
                }
                self.connections[token]
                    .stream
                    .flush()
                    .expect(&format!("Error flushing connection {:?}/{:?}",
                                     token,
                                     self.connections[token]));;
            }
        }

        if events.is_readable() {
            // Read the 6 byte header of each packet, throw it into get_packet_length, then read that number of bytes.
            // Then throw those bytes into deserialize_packet. Afterwards, throw it to handle_packet.

            let mut header = [0; 6];
            self.connections[token]
                .stream
                .read(&mut header)
                .expect(&format!("An error occured reading from socket {:?}", token));    // TODO: Figure out possible errors and take care of them.
            let length = get_packet_length(header).unwrap();    // TODO: Handle error gracefully.
            let mut packet = Vec::new();
            (&mut self.connections[token].stream).take(length as u64).read_to_end(&mut packet).unwrap();
            // I do this because Read.take takes a self, instead of a reasonable alternitive.
            // However &mut Read is it's self a Reader. So I use that instead.
            let dese = deserialize_packet(&packet).unwrap();
            handle_packet(dese, token, event_loop);
        }

        // This may need to be in an if events.is_readable() block.
        if self.listener.is_some() {
            match self.listener.as_ref().unwrap().accept() {
                Ok(Some((socket, address))) => {
                    let token = self.connections.insert(Connection::new(socket)).unwrap();
                    send(event_loop,
                         NetworkPacket::Init { version: VERSION.to_owned() },
                         token);
                    info!("Accepted connection {}.", address);
                }
                Ok(None) => debug!("Recived Ok(None) in listener.accept()."),
                Err(error) => panic!("listener.accept() errored with {:?}!", error),
            }
        }
    }

    fn notify(&mut self, event_loop: &mut MioEventLoop<Handler>, msg: HandlerMessage) {
        match msg {
            HandlerMessage::AddStream(stream, tx) => {
                let token = self.connections.insert(Connection::new(stream)).unwrap();
                event_loop.register(&self.connections[token].stream,
                                    token,
                                    EventSet::all(),
                                    PollOpt::level())
                          .unwrap();
                tx.send(token).unwrap();
            }
            HandlerMessage::Send(packet, token) => {
                self.connections[token].message_queue.push(packet);
            }
        }
    }
}

fn add_socket(event_loop: &EventLoop, socket: TcpStream) -> Token {
    let (tx, rx) = channel();   // How expensive is this? Should I be creating a whole new channel for just 1 message?
    event_loop.channel().send(HandlerMessage::AddStream(socket, tx)).unwrap();
    rx.recv().unwrap() // I should feel bad.
}

/// Send a NetworkPacket over the network.
///
/// This is a static function instead of an impl function because you can't impl on external structs. In this case, mio::EventLoop.
pub fn send(event_loop: &EventLoop, to_send: NetworkPacket, token: Token) {
    event_loop.channel().send(HandlerMessage::Send(to_send, token)).unwrap();
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

#[derive(Debug)]
enum PacketDeseError {
    MagicNumberMismatch,
    InvalidEncoding(DeserializeError),
}

impl From<DeserializeError> for PacketDeseError {
    fn from(err: DeserializeError) -> PacketDeseError {
        PacketDeseError::InvalidEncoding(err)
    }
}

/// Returns the length of a given packet, or an error if the first four bytes do not match NET_MAGIC_NUMBER.
fn get_packet_length(to_ln: [u8; 6]) -> Result<u16, PacketDeseError> {
    let (first_four, next_two) = to_ln.split_at(4);
    let should_be_magic_num = LittleEndian::read_u32(&first_four);
    if should_be_magic_num != NET_MAGIC_NUMBER {
        return Err(PacketDeseError::MagicNumberMismatch);
    }
    let length = LittleEndian::read_u16(&next_two);
    Ok(length)
}

fn deserialize_packet(to_de: &[u8]) -> Result<NetworkPacket, PacketDeseError> {
    Ok(try!(deserialize::<NetworkPacket>(to_de)))
}

fn handle_packet(to_handle: NetworkPacket, sender: Token, event_loop: &EventLoop) {
    match to_handle {
        NetworkPacket::Init{version} => {
            if version != VERSION {
                send(event_loop,
                     NetworkPacket::Error(NetworkError::VersionMismatch(version.to_owned(), VERSION.to_owned())),
                     sender)
            }
        }

        #[cfg(Test)]
        NetworkPacket::Test => unimplemented!(),

        NetworkPacket::Error(error) => if ::check_should_crash() { panic!(error) } else { unimplemented!() },
    }
}
