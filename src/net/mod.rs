//! Contains code relating to networking.

#[cfg(test)]
mod test;

use std::sync::mpsc::SendError;

use mioco;
use mioco::sync::mpsc::{Sender, channel};

use test_util::Tattle;

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
