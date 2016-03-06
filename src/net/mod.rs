//! Netcode for buildengine.
//!
//! At some time, most of this code may be moved into a new crate.

use std::convert::From;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::io::{Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
#[cfg(test)]
use std::sync::atomic::Ordering;
// Since this import is only used while compiling with the test cfg,
// it would cause an unused import warning when compiling in normal mode.
use std::sync::mpsc::{Sender, channel};

use bincode::serde::{DeserializeError, deserialize, serialize};
use bincode::SizeLimit;
use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use mio;
use mio::{EventLoop as MioEventLoop, EventSet, Handler as MioHandler, NotifyError, PollOpt, Token};
use mio::tcp;
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;
use slab::Index;

use VERSION;

pub mod client;

#[cfg(test)]
pub mod test;

/// The maximum nunber of connections that can be had.
///
/// If the number of connections exceeds this number, new connections should be denied.
pub const MAX_CONNECTIONS: usize = 1024;

/// Standard number to ensure network connections are syncronized and the same protocol is being used.
/// Reexported incase it is of use for something not-networking.
pub const NET_MAGIC_NUMBER: u32 = 0xCB011043; //0xcafebade + 0x25565, because programming references.

/// Default port for clients and servers to connect on.
///
/// Mostly exposed for launchers to use, but is also used in unit testing and the like.
pub const STANDARD_PORT: u16 = 25566;

/// A networking event loop. Is supposed to allow the capability to do a number of networking tasks generically.
///
/// Any method taking &self requires that the event loop is running in order to take effect.
/// This is because they rely on message passing to the thread running the loop.
///
/// This is a trait as to make unit testing more unit-y.
pub trait EventLoop: Debug {
    /// Run the loop exactly once.
    fn run_once(&mut self) -> Result<(), io::Error>;
    /// Run the loop forever, or until shutdown() is called.
    fn run(&mut self) -> Result<(), io::Error>;
    /// Stop the event loop after the next iteration.
    fn shutdown(&self);
    /// Send a NetworkPacket over the network.
    fn send(&self, target: Token, packet: NetworkPacket);
    /// Kill a socket at the given token.
    ///
    /// It is important to never use the token of the targer ever. This will cause a panic.
    fn kill(&self, target: Token);
    /// Add a socket to be checked during the event loop.
    ///
    /// Do note that this function assumes that the socket has been properly inited acording to the protocol.
    /// Not doing so could cause hard to discover bugs if versions mismatch.
    fn add_socket(&self, socket: TcpStream) -> Token;
    /// Add a TcpListener to be checked every event loop for new connections.
    fn add_listener(&self, listener: TcpListener);
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

/// Primary impl of EventLoop. Actually does the networking tasks described.
#[derive(Debug)]
pub struct EventLoopImpl {
    mio_event_loop: MioEventLoop<Handler>,
    handler: Handler,
}

impl EventLoopImpl {
    /// Creates a new EventLoopImpl, with the given max_connections.
    ///
    /// When the number of connections accocated with the event loop exceed max_connections, the connection is denied.
    pub fn new(max_connections: usize) -> Result<EventLoopImpl, io::Error> {
        Ok(EventLoopImpl {
            mio_event_loop: try!(MioEventLoop::new()),
            handler: Handler::new(max_connections),
        })
    }
}

impl EventLoop for EventLoopImpl {
    fn run_once(&mut self) -> Result<(), io::Error> {
        self.mio_event_loop.run_once(&mut self.handler, None)
    }

    fn run(&mut self) -> Result<(), io::Error> {
        self.mio_event_loop.run(&mut self.handler)
    }

    fn shutdown(&self) {
        match self.mio_event_loop.channel().send(HandlerMessage::Shutdown) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling shutdown() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling shutdown()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling shutdown()"),
            Ok(val) => val,
        };
    }

    fn send(&self, target: Token, packet: NetworkPacket) {
        match self.mio_event_loop.channel().send(HandlerMessage::Send(target, packet)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling send() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling send()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling send()"),
            Ok(val) => val,
        };
    }

    fn kill(&self, target: Token) {
        match self.mio_event_loop.channel().send(HandlerMessage::Kill(target)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling kill() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling kill()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling kill()"),
            Ok(val) => val,
        };
    }

    fn add_socket(&self, socket: TcpStream) -> Token {
        let (tx, rx) = channel();
        match self.mio_event_loop.channel().send(HandlerMessage::AddSocket(socket, tx)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling add_socket() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling add_socket()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling add_socket()"),
            Ok(val) => val,
        };
        rx.recv().unwrap()
    }

    fn add_listener(&self, listener: TcpListener) {
        match self.mio_event_loop.channel().send(HandlerMessage::AddListener(listener)) {
            Err(NotifyError::Io(err)) => {
                panic!("Io Error while calling add_listener() on event loop: {}",
                       err)
            }
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling add_listener()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling add_listener()"),
            Ok(val) => val,
        };
    }
}

/// Provides an immutable reference to an EventLoopImpl.
///
/// Any method requiring a mutable reference will panic when called.
#[derive(Debug)]
pub struct EventLoopImplRef {
    channel: mio::Sender<HandlerMessage>,
}

impl<'a> From<&'a mut EventLoopImpl> for EventLoopImplRef {
    fn from(from: &'a mut EventLoopImpl) -> Self {
        EventLoopImplRef { channel: from.mio_event_loop.channel() }
    }
}

impl EventLoop for EventLoopImplRef {
    fn run_once(&mut self) -> Result<(), io::Error> {
        panic!("Called run_once() on a EventLoopImplRef. This is an immutable reference and can't use mutable methods.")
    }

    fn run(&mut self) -> Result<(), io::Error> {
        panic!("Called run() on a EventLoopImplRef. This is an immutable reference and can't use mutable methods.")
    }

    fn shutdown(&self) {
        match self.channel.send(HandlerMessage::Shutdown) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling shutdown() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling shutdown()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling shutdown()"),
            Ok(val) => val,
        };
    }

    fn send(&self, target: Token, packet: NetworkPacket) {
        match self.channel.send(HandlerMessage::Send(target, packet)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling send() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling send()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling send()"),
            Ok(val) => val,
        };
    }

    fn kill(&self, target: Token) {
        match self.channel.send(HandlerMessage::Kill(target)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling kill() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling kill()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling kill()"),
            Ok(val) => val,
        };
    }

    fn add_socket(&self, socket: TcpStream) -> Token {
        let (tx, rx) = channel();
        match self.channel.send(HandlerMessage::AddSocket(socket, tx)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling add_socket() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling add_socket()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling add_socket()"),
            Ok(val) => val,
        };
        rx.recv().unwrap()
    }

    fn add_listener(&self, listener: TcpListener) {
        match self.channel.send(HandlerMessage::AddListener(listener)) {
            Err(NotifyError::Io(err)) => {
                panic!("Io Error while calling add_listener() on event loop: {}",
                       err)
            }
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling add_listener()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling add_listener()"),
            Ok(val) => val,
        };
    }
}

/// Like `EventLoopImpl`, but contains the fields by reference, instead of by value.
#[derive(Debug)]
pub struct EventLoopImplMutRef<'a, 'b> {
    mio_event_loop: &'a mut MioEventLoop<Handler>,
    handler: &'b mut Handler,
}

impl<'a, 'b> EventLoopImplMutRef<'a, 'b> {
    /// Helper function for creation of a EventLoopImplMutRef.
    pub fn new(mio_event_loop: &'a mut MioEventLoop<Handler>, handler: &'b mut Handler) -> EventLoopImplMutRef<'a, 'b> {
        EventLoopImplMutRef {
            mio_event_loop: mio_event_loop,
            handler: handler,
        }
    }
}

impl<'a, 'b> EventLoop for EventLoopImplMutRef<'a, 'b> {
    fn run_once(&mut self) -> Result<(), io::Error> {
        self.mio_event_loop.run_once(&mut self.handler, None)
    }

    fn run(&mut self) -> Result<(), io::Error> {
        self.mio_event_loop.run(&mut self.handler)
    }

    fn shutdown(&self) {
        match self.mio_event_loop.channel().send(HandlerMessage::Shutdown) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling shutdown() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling shutdown()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling shutdown()"),
            Ok(val) => val,
        };
    }

    fn send(&self, target: Token, packet: NetworkPacket) {
        match self.mio_event_loop.channel().send(HandlerMessage::Send(target, packet)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling send() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling send()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling send()"),
            Ok(val) => val,
        };
    }

    fn kill(&self, target: Token) {
        match self.mio_event_loop.channel().send(HandlerMessage::Kill(target)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling kill() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling kill()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling kill()"),
            Ok(val) => val,
        };
    }

    fn add_socket(&self, socket: TcpStream) -> Token {
        let (tx, rx) = channel();
        match self.mio_event_loop.channel().send(HandlerMessage::AddSocket(socket, tx)) {
            Err(NotifyError::Io(err)) => panic!("Io Error while calling add_socket() on event loop: {}", err),
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling add_socket()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling add_socket()"),
            Ok(val) => val,
        };
        rx.recv().unwrap()
    }

    fn add_listener(&self, listener: TcpListener) {
        match self.mio_event_loop.channel().send(HandlerMessage::AddListener(listener)) {
            Err(NotifyError::Io(err)) => {
                panic!("Io Error while calling add_listener() on event loop: {}",
                       err)
            }
            Err(NotifyError::Full(_)) => panic!("Event loop channel full while calling add_listener()! Is it running?"),
            Err(NotifyError::Closed(_)) => panic!("Event loop closed while calling add_listener()"),
            Ok(val) => val,
        };
    }
}

/// Keeps the data that is nescary during packet handling.
#[derive(Debug)]
pub struct Handler {
    connections: Slab<Connection>,
    listeners: Vec<TcpListener>,
}

impl Handler {
    /// Creates a new instance of a Handler. Does not listen for connections.
    pub fn new(max_connections: usize) -> Self {
        Handler {
            connections: Slab::new_starting_at(Token::from_usize(1), max_connections),
            listeners: Vec::new(),
        }
    }
}

impl MioHandler for Handler {
    type Timeout = ();
    type Message = HandlerMessage;
    fn ready(&mut self, event_loop: &mut MioEventLoop<Handler>, token: Token, events: EventSet) {
        if events.is_readable() {
            // Read the 6 byte header of each packet, throw it into get_packet_length, then read that number of bytes.
            // Then throw those bytes into deserialize_packet. Afterwards, throw it to handle_packet.

            let mut header = [0; 6];
            self.connections[token]
                .stream
                .read(&mut header)
                .expect(&format!("An error occured reading from socket {:?}", token));    // TODO: Figure out possible errors and take care of them.
            let length = get_packet_length(header).unwrap_or(0);
            if length == 0 {
                EventLoopImplMutRef::new(event_loop, self).kill(token);
                // I directly kill the connection, becasue if the magic number doesn't match,
                // the peer probably doesn't share the same protocol. It wouldn't understand a normal error packet.
                return;
                // Returning pervents other actions from hapening as well.
                // After all, the token is now invalid and will panic or something if left around.
            }
            let mut packet = Vec::new();
            (&mut self.connections[token].stream).take(length as u64).read_to_end(&mut packet).unwrap();
            // I do this because Read.take takes a self, instead of a reasonable alternitive.
            // However &mut Read is it's self a Reader. So I use that instead.
            let dese = deserialize_packet(&packet).unwrap();
            handle_packet(dese, token, &EventLoopImplMutRef::new(event_loop, self));
        }

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
    }

    fn tick(&mut self, event_loop: &mut MioEventLoop<Handler>) {
        let mut to_init: Vec<Token> = Vec::new();
        for listener in &self.listeners {
            match listener.accept() {
                Ok(Some((socket, address))) => {
                    let token = self.connections.insert(Connection::new(socket)).unwrap();
                    to_init.push(token);
                    // Since self is currently borrowed, I can't send the init packet here.
                    // I instead push the work to be done later.
                    info!("Accepted connection {}.", address);
                }
                Ok(None) => debug!("Recived Ok(None) in listener.accept()."),
                Err(error) => panic!("listener.accept() errored with {:?}!", error),
                // TODO: See if any of these errors can be handled better.
            }
        }
        for token in to_init {
            EventLoopImplMutRef::new(event_loop, self).send(token,
                                                            NetworkPacket::Init {
                                                                version: VERSION.to_owned(),
                                                                should_crash: ::check_should_crash(),
                                                            });
        }
    }

    fn notify(&mut self, event_loop: &mut MioEventLoop<Handler>, msg: HandlerMessage) {
        use self::HandlerMessage::*;
        match msg {
            Shutdown => event_loop.shutdown(),
            Send(target, packet) => self.connections[target].message_queue.push(packet),
            Kill(target) => {
                event_loop.deregister(&self.connections[target].stream).expect("io::Error while deregistering socket.");
                self.connections[target].stream.shutdown(tcp::Shutdown::Both).expect("io::Error while shutting down socket.");
                // TODO: Better decern and handle possible errors.
                self.connections.remove(target);
            }
            AddSocket(socket, send) => {
                let token = self.connections.insert(Connection::new(socket)).unwrap();
                event_loop.register(&self.connections[token].stream,
                                    token,
                                    EventSet::all(),
                                    PollOpt::level())
                          .unwrap();
                send.send(token).unwrap();
            }
            AddListener(listener) => self.listeners.push(listener),
        }
    }
}

/// A message to be sent to the handler accocated with a event loop.
#[derive(Debug)]
pub enum HandlerMessage {
    /// Shutdown the eventloop next iteration.
    Shutdown,
    /// Send a network packet to a remote peer.
    Send(Token, NetworkPacket),
    /// Disconnect from a remote peer.
    Kill(Token),
    /// Add a socket to be checked each iteration of the event loop.
    AddSocket(TcpStream, Sender<Token>),
    /// Add a listener to be checked for new connections each iteration fo the event loop.
    AddListener(TcpListener),
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
                       "VersionMismatch: The versions of the client and server attempting to connect mismatch. ver1: {}, ver2: {}",
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
            NetworkError::VersionMismatch(_, _) => "VersionMismatch: The versions of the client and server attempting to connect mismatch.",
            NetworkError::ShouldCrashBothTrue => "ShouldCrashBothTrue: Both peers have should_crash == false.",
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
        /// Two peers should not have this as false, because should_crash will send the error to the remote peer and make it crash instead.
        /// This would cause a infinite loop if both were to do it.
        should_crash: bool,
    },
    /// An error that should crash the game and show an error to the user, but only on a client.
    Error(NetworkError),
    /// Increments a value internally. It's supposed to be used for unit testing.
    #[cfg(test)]
    Test,
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
        panic!("Because localhost can resolve to both 127.0.0.1, and the vairous IPV6 versions of 127.0.0.1, it may not be used. Please instead use 127.0.0.1");
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

fn handle_packet<T: EventLoop>(to_handle: NetworkPacket, sender: Token, event_loop: &T) {
    match to_handle {
        NetworkPacket::Init{version, should_crash} => {
            if !should_crash && !::check_should_crash() {
                event_loop.send(sender,
                                NetworkPacket::Error(NetworkError::ShouldCrashBothTrue));
                event_loop.kill(sender);
            }
            if version != VERSION {
                event_loop.send(sender,
                                NetworkPacket::Error(NetworkError::VersionMismatch(version.to_owned(), VERSION.to_owned())))
            }
        }
        NetworkPacket::Error(error) => if ::check_should_crash() { panic!(error) } else { unimplemented!() },
        #[cfg(test)]
        NetworkPacket::Test => {
            test::TEST_VAL.fetch_add(1, Ordering::Relaxed);
        }
    }
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
