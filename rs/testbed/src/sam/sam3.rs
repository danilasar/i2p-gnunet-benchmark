use std::{
    collections::HashMap,
    error::Error,
    io::{Read, Write},
    net::TcpStream,
};

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

pub struct SamSession {
    _control: TcpStream,
    pub destination: String,
}

impl SamSession {
    pub fn create_stream(
        sam_addr: &str,
        id: &str,
        options: &[(&str, &str)],
    ) -> Result<Self, Box<dyn Error>> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;

        write!(stream, "DEST GENERATE SIGNATURE_TYPE=7\n")?;
        stream.flush()?;
        let line = read_line(&mut stream)?;
        let fields = parse_fields(&line);
        if fields.get("RESULT").is_some_and(|v| v != "OK") {
            return Err(format!("DEST GENERATE failed: {line}").into());
        }
        let pub_dest = fields
            .get("PUB")
            .ok_or_else(|| format!("DEST GENERATE without PUB: {line}"))?
            .to_string();
        let priv_dest = fields
            .get("PRIV")
            .ok_or_else(|| format!("DEST GENERATE without PRIV: {line}"))?
            .to_string();

        write!(
            stream,
            "SESSION CREATE STYLE=STREAM ID={id} DESTINATION={priv_dest} "
        )?;
        for (key, value) in options {
            write!(stream, "{key}={value} ")?;
        }
        write!(stream, "SIGNATURE_TYPE=7\n")?;
        stream.flush()?;
        let line = read_line(&mut stream)?;
        ensure_ok(&line, "SESSION CREATE")?;

        Ok(Self {
            _control: stream,
            destination: pub_dest,
        })
    }

    pub fn connect_stream(
        sam_addr: &str,
        id: &str,
        dest: &str,
    ) -> Result<TcpStream, Box<dyn Error>> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;
        write!(
            stream,
            "STREAM CONNECT ID={id} DESTINATION={dest} FROM_PORT=0 TO_PORT=0 SILENT=false\n"
        )?;
        stream.flush()?;
        let line = read_line(&mut stream)?;
        ensure_ok(&line, "STREAM CONNECT")?;
        Ok(stream)
    }

    pub fn accept_stream(sam_addr: &str, id: &str) -> Result<TcpStream, Box<dyn Error>> {
        let mut stream = TcpStream::connect(sam_addr)?;
        hello(&mut stream)?;
        write!(stream, "STREAM ACCEPT ID={id} SILENT=false\n")?;
        stream.flush()?;
        let line = read_line(&mut stream)?;
        ensure_ok(&line, "STREAM ACCEPT")?;
        let _remote_destination = read_line(&mut stream)?;
        Ok(stream)
    }
}

fn hello(stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    stream.write_all(b"HELLO VERSION MIN=3.0 MAX=3.3\n")?;
    stream.flush()?;
    let line = read_line(stream)?;
    if !line.contains("HELLO REPLY") || !line.contains("RESULT=OK") {
        return Err(format!("HELLO failed: {line}").into());
    }
    Ok(())
}

fn ensure_ok(line: &str, command: &str) -> Result<(), Box<dyn Error>> {
    if line.contains("RESULT=OK") {
        Ok(())
    } else {
        Err(format!("{command} failed: {line}").into())
    }
}

fn read_line(stream: &mut TcpStream) -> Result<String, Box<dyn Error>> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte)?;
        buf.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    Ok(String::from_utf8(buf)?.trim_end().to_string())
}

fn parse_fields(line: &str) -> HashMap<String, String> {
    line.split_whitespace()
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}
