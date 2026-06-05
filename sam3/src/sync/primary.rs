use std::io::BufReader;
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use crate::error::SamError;
use crate::proto::{command, response};
use crate::sync::{read_line, sam_open, write_cmd, SessionOptions};
use crate::sync::stream::{Destination, Keys, SessionKind, StreamSession};
use crate::sync::datagram::DatagramSession;
use crate::sync::raw::RawSession;

pub struct PrimarySession {
    pub(crate) control: Arc<Mutex<BufReader<TcpStream>>>,
    pub(crate) sam_addr: String,
    pub(crate) id: String,
    pub(crate) keys: Keys,
}

impl PrimarySession {
    pub(crate) fn create(sam_addr: &str, id: &str, keys: Keys, opts: &SessionOptions) -> Result<Self, SamError> {
        let mut reader = sam_open(sam_addr)?;

        write_cmd(&mut reader, &command::session_create_primary(id, keys.private_key().as_str(), &opts.to_opts_str()))?;
        
        let line = read_line(&mut reader)?;
        let _ = response::parse_session_status(&line)?;

        Ok(Self {
            control: Arc::new(Mutex::new(reader)),
            sam_addr: sam_addr.to_string(),
            id: id.to_string(),
            keys,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn local_destination(&self) -> &Destination {
        &self.keys.public
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    pub fn new_stream_sub_session(&mut self, id: impl Into<String>) -> Result<StreamSession, SamError> {
        let id = id.into();
        
        let mut reader = self.control.lock().unwrap();
        write_cmd(&mut reader, &command::session_add_stream(&id))?;
        
        let line = read_line(&mut reader)?;
        let dest = response::parse_session_status(&line)?;

        Ok(StreamSession {
            sam_addr: self.sam_addr.clone(),
            id,
            destination: dest.into(),
            keys: self.keys.clone(),
            kind: SessionKind::SubSession(Arc::clone(&self.control)),
        })
    }

    pub fn new_datagram_sub_session(&mut self, id: impl Into<String>) -> Result<DatagramSession, SamError> {
        let id = id.into();
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        let local_port = socket.local_addr()?.port();

        let mut reader = self.control.lock().unwrap();
        write_cmd(&mut reader, &command::session_add_datagram(&id, local_port))?;
        
        let line = read_line(&mut reader)?;
        let _dest = response::parse_session_status(&line)?;

        let sam_udp_addr = std::net::SocketAddr::new(
            reader.get_ref().peer_addr()?.ip(),
            command::SAM_UDP_PORT,
        );

        Ok(DatagramSession {
            _control: None, 
            socket,
            sam_udp_addr,
            id,
            keys: self.keys.clone(),
        })
    }

    pub fn new_raw_sub_session(&mut self, id: impl Into<String>) -> Result<RawSession, SamError> {
        let id = id.into();
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        let local_port = socket.local_addr()?.port();

        let mut reader = self.control.lock().unwrap();
        write_cmd(&mut reader, &command::session_add_raw(&id, local_port))?;
        
        let line = read_line(&mut reader)?;
        let _dest = response::parse_session_status(&line)?;

        let sam_udp_addr = std::net::SocketAddr::new(
            reader.get_ref().peer_addr()?.ip(),
            command::SAM_UDP_PORT,
        );

        Ok(RawSession {
            _control: None,
            socket,
            sam_udp_addr,
            id,
            keys: self.keys.clone(),
        })
    }

    pub fn remove_sub_session(&mut self, id: &str) -> Result<(), SamError> {
        let mut reader = self.control.lock().unwrap();
        write_cmd(&mut reader, &command::session_remove(id))?;
        let line = read_line(&mut reader)?;
        response::check_result(&line)
    }

    pub fn ping(&self, payload: &str) -> Result<String, SamError> {
        let mut reader = self.control.lock().unwrap();
        write_cmd(&mut reader, &command::ping(payload))?;
        let line = read_line(&mut reader)?;
        response::parse_pong(&line)
    }
}
