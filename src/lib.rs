#![feature(custom_derive, plugin, const_fn)]
#![plugin(serde_macros)]
#![deny(missing_docs,
        missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unused_import_braces, unused_qualifications,
        warnings)]

//! Implementation of the build engine. This contains entry points and some misc utils for launchers.

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

extern crate bincode;
extern crate byteorder;
extern crate either;
extern crate env_logger;
extern crate hlua;
extern crate mio;
extern crate serde;
extern crate slab;

pub mod net;
pub mod script;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::error::Error;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::io;
use std::net::SocketAddr;

use mio::tcp::TcpListener;

use net::{EventLoop, EventLoopImpl, MAX_CONNECTIONS, client};
use net::client::Client;

/// The current version of buildengine. Fallows Semantic Versioning.
pub const VERSION: &'static str = "0.0.1";

/// If the game is allowed to crash in the event of a semi-handleable error, such as a bad network packet or a peer crashing.
///
/// Programming mistakes however, will still panic.
static SHOULD_CRASH: AtomicBool = AtomicBool::new(true);    // Basically Erlang's too_big_to_fail process_flag.

/// Main game struct. Contains all state nescary to work.
///
/// While you may never need the fields exposed, they are exposed if you ever want to inspect the game state.
/// You probably, however don't want to mutate the state directly. That can mess up client-server syncronization.
#[derive(Debug)]
pub struct Engine<'be> {
    /// The clientside or serverside networking state.
    ///
    /// Currently a Some if it is a client, or None if server.
    pub net_state: Option<Client>,

    /// The networking event loop. Mostly used in other functions for sending, adding, and killing connections.
    ///
    /// Also contains all state relating to networking.
    pub event_loop: Box<EventLoop>,

    /// The scripting backend for the engine.
    ///
    /// Not present on a client, for security reasons.
    pub script_engine: Option<script::Engine<'be>>,
}

impl<'be> Engine<'be> {
    /// Creates a new client game.
    pub fn new_client(server_address: SocketAddr) -> Result<Self, InitError> {
        let mut event_loop = try!(EventLoopImpl::new(MAX_CONNECTIONS, Vec::new()));
        let client = try!(Client::spawn_client(server_address, &mut event_loop));
        Ok(Engine {
            event_loop: Box::new(event_loop),
            net_state: Some(client),
            script_engine: None,
        })
    }

    /// Creates a new server.
    pub fn new_server(server_address: &SocketAddr,
                      game_scripts: HashMap<String, String>)
                      -> Result<Self, InitError> {
        let event_loop = try!(EventLoopImpl::new(MAX_CONNECTIONS, Vec::new()));
        let listener = try!(TcpListener::bind(server_address));
        event_loop.add_listener(listener);
        Ok(Engine {
            event_loop: Box::new(event_loop),
            net_state: None,
            script_engine: Some(script::Engine::new(game_scripts)),
        })
    }
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

/// Initalizes the global parts of the engine.
///
/// Currently it only inits the logger, but later may do other things.
///
/// #Panics
/// * Calling the function once it has already been called.
pub fn global_init() {
    env_logger::init().expect("Already inited the logging server!");
}

/// Prints "Hello World!" to stdout. Will be removed in future versions.
pub fn print_hello_world() {
    println!("Hello World!");
}

fn check_should_crash() -> bool {
    SHOULD_CRASH.load(Ordering::Relaxed)
}
