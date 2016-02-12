#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]
#![deny(missing_docs,
        missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unused_import_braces, unused_qualifications,
        warnings)]

//! Implementation of the build engine. This contains entry points and some misc utils for launchers.
extern crate bincode;
extern crate byteorder;
extern crate env_logger;
extern crate errorser;
#[macro_use]
extern crate log;
#[macro_use]
extern crate quick_error;
extern crate serde;
extern crate mio;
extern crate either;
extern crate slab;

mod net;

use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::fmt::{Display, Error as FmtError, Formatter};

use net::client::Client;
use net::server::Server;
use net::{EventLoop, Handler, client};

use either::Either;

/// The current version of buildengine. Fallows Semantic Versioning.
pub const VERSION: &'static str = "0.0.1";

/// If the game is allowed to crash in the event of a semi-handleable error, such as a bad network packet or a peer crashing.
///
/// Programming mistakes however, will still panic.
pub static mut should_crash: bool = true;   // Basically Erlang's too_big_to_fail process_flag.

/// Initalizes the global parts of the engine.
///
/// Currently it only inits the logger, but later may do other things.
///
/// #Panics
/// * Calling the function once it has already been called.
pub fn global_init() {
    env_logger::init().expect("Already inited the logging server!");
}

/// An error hapened while initing the game.
#[derive(Debug)]
pub enum InitError {
    /// An error occoured when initalising the client code.
    ClientInitError(client::InitError),
    /// An std::io::Error. Who knows where this comes up.
    IoError(io::Error),
}

impl Display for InitError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            InitError::ClientInitError(ref err) => write!(fmt, "ClientInitError: {}", err),
            InitError::IoError(ref err) => write!(fmt, "IoError: {}", err),
        }
    }
}

impl Error for InitError {
    fn description(&self) -> &str {
        match *self {
            InitError::ClientInitError(ref err) => err.description(),
            InitError::IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            InitError::ClientInitError(ref err) => Some(err),
            InitError::IoError(ref err) => Some(err),
        }
    }
}

impl From<client::InitError> for InitError {
    fn from(err: client::InitError) -> Self {
        InitError::ClientInitError(err)
    }
}

impl From<io::Error> for InitError {
    fn from(err: io::Error) -> Self {
        InitError::IoError(err)
    }
}

struct Engine {
    handler: Handler,
    event_loop: EventLoop,
    client_or_server: Either<Client, Server>,
}

impl Engine {
    /// Creates a new client game.
    pub fn new_client(server_address: SocketAddr) -> Result<Self, InitError> {
        let event_loop = try!(EventLoop::new());
        let handler = Handler::new();
        let client = try!(Client::spawn_client(server_address, &event_loop));
        Ok(Engine {
            handler: handler,
            event_loop: event_loop,
            client_or_server: Either::Left(client),
        })
    }
}

/// Prints "Hello World!" to stdout. Will be removed in future versions.
pub fn print_hello_world() {
    println!("Hello World!");
}

fn check_should_crash() -> bool {
    unsafe {
        should_crash    // Is it really okay to use this value if it will only change when the application is still single threaded?
    }
}
