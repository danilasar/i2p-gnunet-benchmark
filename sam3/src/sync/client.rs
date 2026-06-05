use std::path::Path;
use crate::error::SamError;
use crate::proto::{command, response};
use crate::sync::{generate_keys_on_new_conn, read_line, sam_open, write_cmd, SessionOptions, RawSessionOptions};
use crate::sync::stream::{Destination, Keys, SessionKind, StreamSession};
use crate::sync::datagram::DatagramSession;
use crate::sync::raw::RawSession;
use crate::sync::primary::PrimarySession;

pub struct SamClient {
    sam_addr: String,
}

impl SamClient {
    pub fn connect(addr: impl Into<String>) -> Self {
        Self {
            sam_addr: addr.into(),
        }
    }

    pub fn new_keys(&self) -> Result<Keys, SamError> {
        generate_keys_on_new_conn(&self.sam_addr)
    }

    pub fn new_keys_with_signature_type(&self, sig_type: &str) -> Result<Keys, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;
        write_cmd(&mut reader, &command::dest_generate(sig_type))?;
        let line = read_line(&mut reader)?;
        let (pub_key, priv_key) = response::parse_dest_reply(&line)?;
        Ok(Keys::new(pub_key, priv_key))
    }

    pub fn ensure_keyfile(&self, path: impl AsRef<Path>) -> Result<Keys, SamError> {
        if path.as_ref().exists() {
            Keys::read_keyfile(path)
        } else {
            let keys = self.new_keys()?;
            keys.write_keyfile(path)?;
            Ok(keys)
        }
    }

    pub fn lookup(&self, name: &str) -> Result<Destination, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;
        write_cmd(&mut reader, &command::naming_lookup(name))?;
        let line = read_line(&mut reader)?;
        let dest = response::parse_naming_reply(&line)?;
        Ok(dest.into())
    }

    pub fn ping(&self, payload: &str) -> Result<String, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;
        write_cmd(&mut reader, &command::ping(payload))?;
        let line = read_line(&mut reader)?;
        response::parse_pong(&line)
    }

    pub fn new_stream_session(&self, id: &str, keys: Keys, opts: &SessionOptions) -> Result<StreamSession, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;
        
        write_cmd(&mut reader, &command::session_create_stream(id, keys.private_key().as_str(), &opts.to_opts_str()))?;
        let line = read_line(&mut reader)?;
        let dest = response::parse_session_status(&line)?;

        Ok(StreamSession {
            sam_addr: self.sam_addr.clone(),
            id: id.to_string(),
            destination: dest.into(),
            keys,
            kind: SessionKind::Standalone(reader.into_inner()),
        })
    }

    pub fn new_transient_stream_session(&self, id: &str, opts: &SessionOptions) -> Result<StreamSession, SamError> {
        let mut reader = sam_open(&self.sam_addr)?;
        
        write_cmd(&mut reader, &command::dest_generate(command::DEFAULT_SIGNATURE_TYPE))?;
        let line = read_line(&mut reader)?;
        let (pub_key, priv_key) = response::parse_dest_reply(&line)?;
        let keys = Keys::new(pub_key, priv_key);

        write_cmd(&mut reader, &command::session_create_stream(id, keys.private_key().as_str(), &opts.to_opts_str()))?;
        let line = read_line(&mut reader)?;
        let dest = response::parse_session_status(&line)?;

        Ok(StreamSession {
            sam_addr: self.sam_addr.clone(),
            id: id.to_string(),
            destination: dest.into(),
            keys,
            kind: SessionKind::Standalone(reader.into_inner()),
        })
    }

    pub fn new_datagram_session(&self, id: &str, keys: Keys, opts: &SessionOptions) -> Result<DatagramSession, SamError> {
        DatagramSession::create(&self.sam_addr, id, keys, opts)
    }

    pub fn new_transient_datagram_session(&self, id: &str, opts: &SessionOptions) -> Result<DatagramSession, SamError> {
        let keys = self.new_keys()?;
        self.new_datagram_session(id, keys, opts)
    }

    pub fn new_raw_session(&self, id: &str, keys: Keys, opts: &SessionOptions, raw_opts: &RawSessionOptions) -> Result<RawSession, SamError> {
        RawSession::create(&self.sam_addr, id, keys, opts, raw_opts)
    }

    pub fn new_transient_raw_session(&self, id: &str, opts: &SessionOptions) -> Result<RawSession, SamError> {
        let keys = self.new_keys()?;
        self.new_raw_session(id, keys, opts, &RawSessionOptions::default())
    }

    pub fn new_primary_session(&self, id: &str, keys: Keys, opts: &SessionOptions) -> Result<PrimarySession, SamError> {
        PrimarySession::create(&self.sam_addr, id, keys, opts)
    }

    pub fn new_transient_primary_session(&self, id: &str, opts: &SessionOptions) -> Result<PrimarySession, SamError> {
        let keys = self.new_keys()?;
        self.new_primary_session(id, keys, opts)
    }
}
