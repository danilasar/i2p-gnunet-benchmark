use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use crate::error::SamError;
use crate::proto::command;
use crate::proto::response;
use crate::proto::state::SessionController;

pub(crate) mod options;
pub(crate) mod client;
pub(crate) mod stream;
pub(crate) mod datagram;
pub(crate) mod raw;
pub(crate) mod primary;

pub use client::SamClient;
pub use options::{SessionOptions, StreamConnectOptions, RawSessionOptions};
pub use stream::{ForwardGuard, Incoming, SamConn, StreamListener, StreamSession, Keys, Destination, PrivateKey};
pub use datagram::DatagramSession;
pub use raw::RawSession;
pub use primary::PrimarySession;

/// Performs HELLO handshake on an existing TCP stream.
pub(crate) fn sam_handshake(stream: TcpStream) -> Result<BufReader<TcpStream>, SamError> {
    let mut reader = BufReader::new(stream);
    let mut ctrl = SessionController::new();

    ctrl.begin_handshake()?;
    write_cmd(&mut reader, &command::hello())?;

    let line = read_line(&mut reader)?;
    ctrl.on_hello_reply(&line)?;

    Ok(reader)
}

/// Opens TCP connection to SAM and performs HELLO handshake.
pub(crate) fn sam_open(addr: &str) -> Result<BufReader<TcpStream>, SamError> {
    sam_handshake(TcpStream::connect(addr)?)
}

/// Reads one line from BufReader and trims end.
pub(crate) fn read_line(reader: &mut BufReader<TcpStream>) -> Result<String, SamError> {
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    Ok(buf.trim_end().to_string())
}

/// Writes command string to TcpStream inside BufReader.
pub(crate) fn write_cmd(reader: &mut BufReader<TcpStream>, cmd: &str) -> Result<(), SamError> {
    let stream = reader.get_mut();
    stream.write_all(cmd.as_bytes())?;
    stream.flush()?;
    Ok(())
}

/// Opens connection to SAM and generates keys (DEST GENERATE).
pub(crate) fn generate_keys_on_new_conn(addr: &str) -> Result<Keys, SamError> {
    let mut reader = sam_open(addr)?;
    write_cmd(&mut reader, &command::dest_generate(command::DEFAULT_SIGNATURE_TYPE))?;
    
    let line = read_line(&mut reader)?;
    let (pub_key, priv_key) = response::parse_dest_reply(&line)?;
    
    Ok(Keys {
        public: pub_key.into(),
        private: priv_key.into(),
    })
}
