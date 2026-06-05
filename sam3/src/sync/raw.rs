use std::io;
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;
use crate::error::SamError;
use crate::proto::{command, response};
use crate::sync::{read_line, sam_open, write_cmd, SessionOptions, RawSessionOptions};
use crate::sync::stream::{Destination, Keys};

pub struct RawSession {
    pub(crate) _control: Option<TcpStream>,
    pub(crate) socket: UdpSocket,
    pub(crate) sam_udp_addr: SocketAddr,
    pub(crate) id: String,
    pub(crate) keys: Keys,
}

impl RawSession {
    pub(crate) fn create(
        sam_addr: &str,
        id: &str,
        keys: Keys,
        opts: &SessionOptions,
        raw_opts: &RawSessionOptions,
    ) -> Result<Self, SamError> {
        let mut reader = sam_open(sam_addr)?;

        let socket = UdpSocket::bind("127.0.0.1:0")?;
        let local_port = socket.local_addr()?.port();

        write_cmd(
            &mut reader,
            &command::session_create_raw(
                id,
                keys.private_key().as_str(),
                local_port,
                raw_opts.protocol,
                raw_opts.header,
                &opts.to_opts_str(),
            ),
        )?;
        
        let line = read_line(&mut reader)?;
        let _ = response::parse_session_status(&line)?;

        let sam_udp_addr = SocketAddr::new(
            reader.get_ref().peer_addr()?.ip(),
            command::SAM_UDP_PORT,
        );

        Ok(Self {
            _control: Some(reader.into_inner()),
            socket,
            sam_udp_addr,
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

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn sam_udp_addr(&self) -> SocketAddr {
        self.sam_udp_addr
    }

    pub fn set_sam_udp_addr(&mut self, addr: SocketAddr) {
        self.sam_udp_addr = addr;
    }

    pub fn set_sam_udp_port(&mut self, port: u16) {
        self.sam_udp_addr.set_port(port);
    }

    pub fn send_to(&self, buf: &[u8], dest: &Destination) -> Result<usize, SamError> {
        if buf.len() > 32_768 {
            return Err(SamError::DatagramTooLarge { max: 32_768, got: buf.len() });
        }
        let header = command::raw_header(&self.id, dest.as_str());
        let mut packet = Vec::with_capacity(header.len() + buf.len());
        packet.extend_from_slice(header.as_bytes());
        packet.extend_from_slice(buf);
        
        self.socket.send_to(&packet, self.sam_udp_addr)?;
        Ok(buf.len())
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SamError> {
        loop {
            let (n, addr) = self.socket.recv_from(buf)?;
            if addr.ip() != self.sam_udp_addr.ip() {
                continue;
            }
            return Ok(n);
        }
    }

    pub fn set_read_timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(d)
    }

    pub fn set_write_timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.socket.set_write_timeout(d)
    }
}
