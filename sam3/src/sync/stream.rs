use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::error::SamError;
use crate::proto::command;
use crate::proto::response;
use crate::sync::{read_line, sam_handshake, sam_open, write_cmd, StreamConnectOptions};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Destination(String);

impl Destination {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for Destination {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Destination {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateKey(String);

impl PrivateKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for PrivateKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for PrivateKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    pub(crate) public: Destination,
    pub(crate) private: PrivateKey,
}

impl Keys {
    pub fn new(public: impl Into<Destination>, private: impl Into<PrivateKey>) -> Self {
        Self {
            public: public.into(),
            private: private.into(),
        }
    }

    pub fn destination(&self) -> &Destination {
        &self.public
    }

    pub fn private_key(&self) -> &PrivateKey {
        &self.private
    }

    pub fn write_keyfile(&self, path: impl AsRef<Path>) -> Result<(), SamError> {
        fs::write(
            path,
            format!("PUB={}\nPRIV={}\n", self.destination(), self.private_key()),
        )?;
        Ok(())
    }

    pub fn read_keyfile(path: impl AsRef<Path>) -> Result<Self, SamError> {
        let contents = fs::read_to_string(path)?;
        let mut pub_key = None;
        let mut priv_key = None;
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("PUB=") {
                pub_key = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("PRIV=") {
                priv_key = Some(rest.trim().to_string());
            }
        }
        let pub_key = pub_key.ok_or_else(|| SamError::UnexpectedResponse("missing PUB field".into()))?;
        let priv_key = priv_key.ok_or_else(|| SamError::UnexpectedResponse("missing PRIV field".into()))?;
        Ok(Self::new(pub_key, priv_key))
    }
}

#[derive(Debug)]
pub struct SamConn {
    pub(crate) inner: TcpStream,
    pub(crate) local: Destination,
    pub(crate) remote: Destination,
}

impl SamConn {
    pub fn local_destination(&self) -> &Destination {
        &self.local
    }

    pub fn remote_destination(&self) -> &Destination {
        &self.remote
    }

    pub fn set_read_timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(d)
    }

    pub fn set_write_timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.inner.set_write_timeout(d)
    }

    pub fn set_timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(d)?;
        self.inner.set_write_timeout(d)
    }
}

impl Read for SamConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for SamConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[derive(Debug)]
pub(crate) enum SessionKind {
    Standalone(#[allow(dead_code)] TcpStream),
    SubSession(#[allow(dead_code)] Arc<Mutex<std::io::BufReader<TcpStream>>>),
}

#[derive(Debug)]
pub struct StreamSession {
    pub(crate) sam_addr: String,
    pub(crate) id: String,
    pub(crate) destination: Destination,
    pub(crate) keys: Keys,
    #[allow(dead_code)]
    pub(crate) kind: SessionKind,
}

impl StreamSession {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn sam_addr(&self) -> &str {
        &self.sam_addr
    }

    pub fn destination(&self) -> &Destination {
        &self.destination
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    pub fn lookup(&self, name: &str) -> Result<Destination, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;
        write_cmd(&mut reader, &command::naming_lookup(name))?;
        let line = read_line(&mut reader)?;
        let dest = response::parse_naming_reply(&line)?;
        Ok(dest.into())
    }

    pub fn dial(&self, dest: &str) -> Result<SamConn, SamError> {
        self.dial_with_options(dest, &StreamConnectOptions::default())
    }

    pub fn dial_timeout(&self, dest: &str, timeout: Duration) -> Result<SamConn, SamError> {
        let opts = StreamConnectOptions::default();
        let stream = TcpStream::connect(&self.sam_addr)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        
        let mut reader = sam_handshake(stream)?;

        write_cmd(&mut reader, &command::stream_connect(&self.id, dest, opts.silent, opts.from_port, opts.to_port))?;
        
        let line = read_line(&mut reader)?;
        response::parse_stream_status(&line)?;

        Ok(SamConn {
            inner: reader.into_inner(),
            local: self.destination.clone(),
            remote: dest.into(),
        })
    }

    pub fn dial_with_options(&self, dest: &str, opts: &StreamConnectOptions) -> Result<SamConn, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;

        write_cmd(&mut reader, &command::stream_connect(&self.id, dest, opts.silent, opts.from_port, opts.to_port))?;
        
        let line = read_line(&mut reader)?;
        response::parse_stream_status(&line)?;

        Ok(SamConn {
            inner: reader.into_inner(),
            local: self.destination.clone(),
            remote: dest.into(),
        })
    }

    pub fn listen(&self) -> StreamListener {
        StreamListener {
            sam_addr: self.sam_addr.clone(),
            id: self.id.clone(),
            local: self.destination.clone(),
        }
    }

    pub fn forward(&self, port: u16, silent: bool) -> Result<ForwardGuard, SamError> {
        self.forward_to(port, None, silent)
    }

    pub fn forward_to(&self, port: u16, host: Option<&str>, silent: bool) -> Result<ForwardGuard, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;

        write_cmd(&mut reader, &command::stream_forward(&self.id, port, host, silent))?;
        
        let line = read_line(&mut reader)?;
        response::parse_stream_status(&line)?;

        Ok(ForwardGuard(reader.into_inner()))
    }
}

#[derive(Debug)]
pub struct StreamListener {
    pub(crate) sam_addr: String,
    pub(crate) id: String,
    pub(crate) local: Destination,
}

impl StreamListener {
    pub fn accept(&self) -> Result<SamConn, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;

        write_cmd(&mut reader, &command::stream_accept(&self.id, false))?;
        
        let line = read_line(&mut reader)?;
        response::parse_stream_status(&line)?;
        
        let peer_line = read_line(&mut reader)?;
        let remote_dest = response::parse_stream_peer(&peer_line)?;

        Ok(SamConn {
            inner: reader.into_inner(),
            local: self.local.clone(),
            remote: remote_dest.into(),
        })
    }

    pub fn accept_timeout(&self, timeout: Duration) -> Result<SamConn, SamError> {
        let stream = TcpStream::connect(&self.sam_addr)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let mut reader = sam_handshake(stream)?;

        write_cmd(&mut reader, &command::stream_accept(&self.id, false))?;
        
        let line = read_line(&mut reader)?;
        response::parse_stream_status(&line)?;
        
        let peer_line = read_line(&mut reader)?;
        let remote_dest = response::parse_stream_peer(&peer_line)?;

        Ok(SamConn {
            inner: reader.into_inner(),
            local: self.local.clone(),
            remote: remote_dest.into(),
        })
    }

    pub fn incoming(&self) -> Incoming<'_> {
        Incoming { listener: self }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug)]
pub struct ForwardGuard(#[allow(dead_code)] pub(crate) TcpStream);

pub struct Incoming<'a> {
    pub(crate) listener: &'a StreamListener,
}

impl<'a> Iterator for Incoming<'a> {
    type Item = Result<SamConn, SamError>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
}
