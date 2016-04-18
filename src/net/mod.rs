//! Contains code relating to networking.

use std::sync::mpsc::SendError;

use mioco;
use mioco::sync::mpsc::{Sender, channel};

/// Represents the network state and provides various utilities acting upon it.
#[allow(missing_debug_implementations)]
pub struct NetHandle(Sender<()>);

impl NetHandle {
    /// Construct a new instance, starting a new coroutine and opening all network traffic on the spesified port.
    pub fn new() -> Self {
        let (tx, rx) = channel::<()>(); // TODO: Maybe have more possible messages than () to shutdown?
        mioco::spawn(move || {
            loop {
                select!(
                    rx:r => {
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
