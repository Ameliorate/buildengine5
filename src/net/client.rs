//! Client side netcode. Creates the types nescary for creating a client and storing it's state.

use std::convert::From;
use std::error::Error;
use std::io;
use std::fmt;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::fmt::Display;

use net::{EventLoop, NetworkPacket};
use VERSION;
use mio::Token;
use mio::tcp::TcpStream;

/// An error that can occour initalising the client.
#[derive(Debug)]
pub enum InitError {
    /// An io::Error.
    IoError(io::Error),
}

impl Display for InitError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            InitError::IoError(ref err) => write!(fmt, "IoError: {}", err),
        }
    }
}

impl Error for InitError {
    fn description(&self) -> &str {
        match *self {
            InitError::IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            InitError::IoError(ref err) => Some(err),
        }
    }
}

impl From<io::Error> for InitError {
    fn from(err: io::Error) -> InitError {
        InitError::IoError(err)
    }
}

/// The state for a client.
#[derive(Debug, Clone, Copy)]
pub struct Client {
    /// The server the client is connected to,
    pub token: Token,
}

impl Client {
    /// Creates a client and connects to the remote server.
    pub fn spawn_client<T: EventLoop>(server_address: SocketAddr,
                                      event_loop: &T)
                                      -> Result<Client, InitError> {
        let socket = try!(TcpStream::connect(&server_address));
        let token = event_loop.add_socket(socket);
        event_loop.send(token,
                        NetworkPacket::Init {
                            version: VERSION.to_string(),
                            should_crash: ::check_should_crash(),
                        });
        Ok(Client { token: token })
    }

    /// Shutdown the connection to the server accocated with this client.
    pub fn shutdown<T: EventLoop>(self, event_loop: &T) {
        event_loop.kill(self.token);
    }

    /// Send a message to the server accocated with the client.
    pub fn send<T: EventLoop>(&self, event_loop: &T, packet: NetworkPacket) {
        event_loop.send(self.token, packet);
    }
}
