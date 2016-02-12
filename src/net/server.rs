use mio::Token;

#[derive(Debug)]
pub enum InitError {
    UnknownError,
}

pub struct Server {
    token: Token,
}

impl Server {
    fn new(port: u16) -> Result<Server, InitError> {
        unimplemented!();
    }
}
