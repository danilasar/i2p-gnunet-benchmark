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
    use super::{ensure_ok, parse_fields};

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
        let err = ensure_ok("STREAM STATUS RESULT=CANT_REACH_PEER", "STREAM CONNECT")
            .expect_err("non-OK result must fail");

        assert!(err.to_string().contains("STREAM CONNECT failed"));
    }
}
