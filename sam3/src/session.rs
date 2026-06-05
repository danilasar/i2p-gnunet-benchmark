use std::{
    collections::HashMap,
    fmt, fs,
    io::{self, Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

#[derive(Debug)]
pub struct SamConn {
    inner: TcpStream,
    local: Destination,
    remote: Destination,
}

impl SamConn {
    pub fn new(inner: TcpStream, local: Destination, remote: Destination) -> Self {
        Self {
            inner,
            local,
            remote,
        }
    }

    pub fn local_destination(&self) -> &Destination {
        &self.local
    }

    pub fn remote_destination(&self) -> &Destination {
        &self.remote
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(dur)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_write_timeout(dur)
    }

    pub fn set_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(dur)?;
        self.inner.set_write_timeout(dur)
    }
}

impl io::Read for SamConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl io::Write for SamConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub const DEFAULT_SIGNATURE_TYPE: &str = "7";

pub const SAM_TUNNEL_OPTIONS: &[(&str, &str)] = &[
    ("inbound.length", "0"),
    ("outbound.length", "0"),
    ("inbound.lengthVariance", "0"),
    ("outbound.lengthVariance", "0"),
    ("inbound.backupQuantity", "0"),
    ("outbound.backupQuantity", "0"),
    ("inbound.quantity", "2"),
    ("outbound.quantity", "2"),
];

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
    public: Destination,
    private: PrivateKey,
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

    pub fn write_keyfile(&self, path: impl AsRef<Path>) -> Result<(), crate::SamError> {
        fs::write(
            path,
            format!("PUB={}\nPRIV={}\n", self.destination(), self.private_key()),
        )?;
        Ok(())
    }

    pub fn read_keyfile(path: impl AsRef<Path>) -> Result<Self, crate::SamError> {
        let contents = fs::read_to_string(path)?;
        if let Some(keys) = parse_keyfile(&contents) {
            return Ok(keys);
        }
        Ok(Self::new("", contents.trim_end()))
    }

    pub fn with_destination(mut self, destination: impl Into<Destination>) -> Self {
        self.public = destination.into();
        self
    }
}

#[derive(Debug)]
pub struct SamSession {
    _control: TcpStream,
    keys: Keys,
}

#[derive(Debug, Clone)]
pub struct SamClient {
    sam_addr: String,
}

impl SamClient {
    pub fn connect(sam_addr: impl Into<String>) -> Self {
        Self {
            sam_addr: sam_addr.into(),
        }
    }

    pub fn new_stream_session(
        &self,
        id: impl Into<String>,
        keys: &Keys,
        options: &[(&str, &str)],
    ) -> Result<StreamSession, crate::SamError> {
        let id = id.into();
        let session = SamSession::create_stream_with_keys(&self.sam_addr, &id, keys, options)?;
        Ok(StreamSession {
            sam_addr: self.sam_addr.clone(),
            id,
            session,
        })
    }

    pub fn new_transient_stream_session(
        &self,
        id: impl Into<String>,
        options: &[(&str, &str)],
    ) -> Result<StreamSession, crate::SamError> {
        let id = id.into();
        let session = SamSession::create_stream(&self.sam_addr, &id, options)?;
        Ok(StreamSession {
            sam_addr: self.sam_addr.clone(),
            id,
            session,
        })
    }

    pub fn new_keys(&self) -> Result<Keys, crate::SamError> {
        self.new_keys_with_signature_type(DEFAULT_SIGNATURE_TYPE)
    }

    pub fn new_keys_with_signature_type(&self, sig_type: &str) -> Result<Keys, crate::SamError> {
        let mut stream = TcpStream::connect(&self.sam_addr)?;
        hello(&mut stream)?;
        generate_keys_on(&mut stream, sig_type)
    }

    pub fn ensure_keyfile(&self, path: impl AsRef<Path>) -> Result<Keys, crate::SamError> {
        let path = path.as_ref();
        if path.exists() {
            Keys::read_keyfile(path)
        } else {
            let keys = self.new_keys()?;
            keys.write_keyfile(path)?;
            Ok(keys)
        }
    }

    pub fn lookup(&self, name: &str) -> Result<Destination, crate::SamError> {
        let mut stream = TcpStream::connect(&self.sam_addr)?;
        hello(&mut stream)?;
        lookup_on(&mut stream, name)
    }
}

#[derive(Debug)]
pub struct StreamSession {
    sam_addr: String,
    id: String,
    session: SamSession,
}

impl StreamSession {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn sam_addr(&self) -> &str {
        &self.sam_addr
    }

    pub fn destination(&self) -> &Destination {
        self.session.destination()
    }

    pub fn keys(&self) -> &Keys {
        self.session.keys()
    }

    pub fn lookup(&self, name: &str) -> Result<Destination, crate::SamError> {
        let mut stream = TcpStream::connect(&self.sam_addr)?;
        hello(&mut stream)?;
        lookup_on(&mut stream, name)
    }

    pub fn dial(&self, dest: &str) -> Result<SamConn, crate::SamError> {
        let stream = SamSession::connect_stream(&self.sam_addr, &self.id, dest)?;
        Ok(SamConn::new(
            stream,
            self.destination().clone(),
            Destination::new(dest),
        ))
    }

    pub fn dial_timeout(&self, dest: &str, timeout: Duration) -> Result<SamConn, crate::SamError> {
        let stream = connect_stream_timeout(&self.sam_addr, &self.id, dest, timeout)?;
        Ok(SamConn::new(
            stream,
            self.destination().clone(),
            Destination::new(dest),
        ))
    }

    pub fn listen(&self) -> StreamListener {
        StreamListener {
            sam_addr: self.sam_addr.clone(),
            id: self.id.clone(),
            local: self.destination().clone(),
        }
    }
}

pub struct StreamListener {
    sam_addr: String,
    id: String,
    local: Destination,
}

impl StreamListener {
    pub fn accept(&self) -> Result<SamConn, crate::SamError> {
        SamSession::accept_stream(&self.sam_addr, &self.id, &self.local)
    }

    pub fn accept_timeout(&self, timeout: Duration) -> Result<SamConn, crate::SamError> {
        accept_stream_timeout(&self.sam_addr, &self.id, &self.local, timeout)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl SamSession {
    pub fn create_stream(
        sam_addr: &str,
        id: &str,
        options: &[(&str, &str)],
    ) -> Result<Self, crate::SamError> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;
        let keys = generate_keys_on(&mut stream, DEFAULT_SIGNATURE_TYPE)?;
        create_stream_on(stream, id, &keys, options)
    }

    pub fn create_stream_with_keys(
        sam_addr: &str,
        id: &str,
        keys: &Keys,
        options: &[(&str, &str)],
    ) -> Result<Self, crate::SamError> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;
        create_stream_on(stream, id, keys, options)
    }

    pub fn connect_stream(
        sam_addr: &str,
        id: &str,
        dest: &str,
    ) -> Result<TcpStream, crate::SamError> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;
        write!(
            stream,
            "STREAM CONNECT ID={id} DESTINATION={dest} FROM_PORT=0 TO_PORT=0 SILENT=false\n"
        )?;
        stream.flush()?;
        let line = read_line(&mut stream)?;
        ensure_ok(&line)?;
        Ok(stream)
    }

    pub fn accept_stream(
        sam_addr: &str,
        id: &str,
        local: &Destination,
    ) -> Result<SamConn, crate::SamError> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;
        write!(stream, "STREAM ACCEPT ID={id} SILENT=false\n")?;
        stream.flush()?;
        let line = read_line(&mut stream)?;
        ensure_ok(&line)?;
        let remote = Destination::new(read_line(&mut stream)?);
        Ok(SamConn::new(stream, local.clone(), remote))
    }

    pub fn destination(&self) -> &Destination {
        self.keys.destination()
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }
}

fn connect_stream_timeout(
    sam_addr: &str,
    id: &str,
    dest: &str,
    timeout: Duration,
) -> Result<TcpStream, crate::SamError> {
    let mut stream = TcpStream::connect(sam_addr)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    hello(&mut stream)?;
    write!(
        stream,
        "STREAM CONNECT ID={id} DESTINATION={dest} FROM_PORT=0 TO_PORT=0 SILENT=false\n"
    )?;
    stream.flush()?;
    let line = read_line(&mut stream)?;
    ensure_ok(&line)?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok(stream)
}

fn accept_stream_timeout(
    sam_addr: &str,
    id: &str,
    local: &Destination,
    timeout: Duration,
) -> Result<SamConn, crate::SamError> {
    let mut stream = TcpStream::connect(sam_addr)?;
    hello(&mut stream)?;
    write!(stream, "STREAM ACCEPT ID={id} SILENT=false\n")?;
    stream.flush()?;
    let line = read_line(&mut stream)?;
    ensure_ok(&line)?;
    stream.set_read_timeout(Some(timeout))?;
    let remote = Destination::new(read_line(&mut stream)?);
    stream.set_read_timeout(None)?;
    Ok(SamConn::new(stream, local.clone(), remote))
}

fn generate_keys_on(stream: &mut TcpStream, sig_type: &str) -> Result<Keys, crate::SamError> {
    write!(stream, "DEST GENERATE SIGNATURE_TYPE={sig_type}\n")?;
    stream.flush()?;
    let line = read_line(stream)?;
    let fields = parse_fields(&line);
    if fields.get("RESULT").is_some_and(|v| v != "OK") {
        return Err(crate::SamError::from_result_line(&line));
    }
    let pub_dest = fields
        .get("PUB")
        .ok_or_else(|| crate::SamError::UnexpectedResponse(format!("DEST GENERATE without PUB: {line}")))?
        .to_string();
    let priv_dest = fields
        .get("PRIV")
        .ok_or_else(|| crate::SamError::UnexpectedResponse(format!("DEST GENERATE without PRIV: {line}")))?
        .to_string();
    Ok(Keys::new(pub_dest, priv_dest))
}

fn create_stream_on(
    mut stream: TcpStream,
    id: &str,
    keys: &Keys,
    options: &[(&str, &str)],
) -> Result<SamSession, crate::SamError> {
    write!(
        stream,
        "SESSION CREATE STYLE=STREAM ID={id} DESTINATION={} ",
        keys.private_key()
    )?;
    for (key, value) in options {
        write!(stream, "{key}={value} ")?;
    }
    write!(stream, "SIGNATURE_TYPE={DEFAULT_SIGNATURE_TYPE}\n")?;
    stream.flush()?;
    let line = read_line(&mut stream)?;
    ensure_ok(&line)?;

    Ok(SamSession {
        _control: stream,
        keys: keys.clone(),
    })
}

fn parse_keyfile(contents: &str) -> Option<Keys> {
    let mut public = None;
    let mut private = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("PUB=") {
            public = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("PRIV=") {
            private = Some(value.to_string());
        }
    }
    Some(Keys::new(public?, private?))
}

fn hello(stream: &mut TcpStream) -> Result<(), crate::SamError> {
    stream.write_all(b"HELLO VERSION MIN=3.0 MAX=3.3\n")?;
    stream.flush()?;
    let line = read_line(stream)?;
    if !line.contains("HELLO REPLY") || !line.contains("RESULT=OK") {
        return Err(crate::SamError::from_result_line(&line));
    }
    Ok(())
}

fn lookup_on(stream: &mut TcpStream, name: &str) -> Result<Destination, crate::SamError> {
    write!(stream, "NAMING LOOKUP NAME={name}\n")?;
    stream.flush()?;
    let line = read_line(stream)?;
    let fields = parse_fields(&line);
    match fields.get("RESULT").map(String::as_str) {
        Some("OK") => {
            let value = fields.get("VALUE").ok_or_else(|| {
                crate::SamError::UnexpectedResponse(format!("NAMING REPLY without VALUE: {line}"))
            })?;
            Ok(Destination::new(value))
        }
        _ => Err(crate::SamError::from_result_line(&line)),
    }
}

pub(crate) fn ensure_ok(line: &str) -> Result<(), crate::SamError> {
    let fields = parse_fields(line);
    if fields.get("RESULT").is_some_and(|v| v == "OK") {
        Ok(())
    } else {
        Err(crate::SamError::from_result_line(line))
    }
}

fn read_line(stream: &mut TcpStream) -> Result<String, crate::SamError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte)?;
        buf.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    Ok(String::from_utf8(buf).map_err(|e| crate::SamError::UnexpectedResponse(e.to_string()))?.trim_end().to_string())
}

pub(crate) fn parse_fields(line: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut parts = line.split_whitespace().peekable();

    while let Some(part) = parts.next() {
        let Some((key, raw_value)) = part.split_once('=') else {
            continue;
        };

        if let Some(stripped) = raw_value.strip_prefix('"') {
            let mut value = String::from(stripped);
            while !ends_with_unescaped_quote(&value) {
                let Some(next) = parts.next() else {
                    break;
                };
                value.push(' ');
                value.push_str(next);
            }
            if ends_with_unescaped_quote(&value) {
                value.pop();
            }
            fields.insert(key.to_string(), unescape_quoted(&value));
        } else {
            fields.insert(key.to_string(), raw_value.to_string());
        }
    }

    fields
}

fn ends_with_unescaped_quote(value: &str) -> bool {
    if !value.ends_with('"') {
        return false;
    }

    let backslashes = value
        .as_bytes()
        .iter()
        .rev()
        .skip(1)
        .take_while(|&&b| b == b'\\')
        .count();
    backslashes % 2 == 0
}

fn unescape_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    if escaped {
        out.push('\\');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{ensure_ok, parse_fields, Keys};

    #[test]
    fn parse_fields_reads_basic_key_values() {
        let fields = parse_fields("HELLO REPLY RESULT=OK VERSION=3.3");

        assert_eq!(fields.get("RESULT").map(String::as_str), Some("OK"));
        assert_eq!(fields.get("VERSION").map(String::as_str), Some("3.3"));
        assert!(!fields.contains_key("HELLO"));
    }

    #[test]
    fn parse_fields_reads_quoted_values_with_spaces() {
        let fields = parse_fields(r#"STREAM STATUS RESULT=I2P_ERROR MESSAGE="cannot reach peer""#);

        assert_eq!(
            fields.get("MESSAGE").map(String::as_str),
            Some("cannot reach peer")
        );
    }

    #[test]
    fn parse_fields_unescapes_quoted_values() {
        let fields = parse_fields(r#"X Y MESSAGE="bad \"quoted\" value" PATH="a\\b""#);

        assert_eq!(
            fields.get("MESSAGE").map(String::as_str),
            Some(r#"bad "quoted" value"#)
        );
        assert_eq!(fields.get("PATH").map(String::as_str), Some(r#"a\b"#));
    }

    #[test]
    fn ensure_ok_rejects_non_ok_results() {
        let err = ensure_ok("STREAM STATUS RESULT=CANT_REACH_PEER")
            .expect_err("non-OK result must fail");

        assert_eq!(err, crate::SamError::CantReachPeer);
    }

    #[test]
    fn keys_roundtrip_keyfile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("keys.dat");
        let keys = Keys::new("pubdest", "privdest");

        keys.write_keyfile(&path).expect("write keyfile");
        let loaded = Keys::read_keyfile(&path).expect("read keyfile");

        assert_eq!(loaded, keys);
    }

    #[test]
    fn keys_read_private_only_keyfile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("keys.dat");
        std::fs::write(&path, "privdest\n").expect("write raw keyfile");

        let loaded = Keys::read_keyfile(&path).expect("read keyfile");

        assert_eq!(loaded.destination().as_str(), "");
        assert_eq!(loaded.private_key().as_str(), "privdest");
    }
}
