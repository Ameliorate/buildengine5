use std::convert::From;
use std::error::Error;
use std::io;
use std::fmt;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::fmt::Display;

use net::{EventLoop, NetworkPacket, add_socket, send};
use VERSION;
use mio::Token;
use mio::tcp::TcpStream;

#[derive(Debug)]
pub enum InitError {
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

pub struct Client {
    token: Token,
}

impl Client {
    pub fn spawn_client(server_address: SocketAddr, event_loop: &EventLoop) -> Result<Client, InitError> {
        let socket = try!(TcpStream::connect(&server_address));
        let token = add_socket(event_loop, socket);
        send(event_loop,
             NetworkPacket::Init { version: VERSION.to_string() },
             token);
        Ok(Client { token: token })
    }
}
