use std::collections::HashMap;
use crate::error::SamError;

/// Parses SAM response string into HashMap<key, value>.
pub fn parse_fields(line: &str) -> HashMap<String, String> {
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

/// Checks if RESULT=OK. Returns SamError::from_result_line(line) on error.
pub fn check_result(line: &str) -> Result<(), SamError> {
    let fields = parse_fields(line);
    if fields.get("RESULT").map(|s| s.as_str()) == Some("OK") {
        Ok(())
    } else {
        Err(SamError::from_result_line(line))
    }
}

/// Parses HELLO REPLY RESULT=OK VERSION=x.
pub fn parse_hello(line: &str) -> Result<(), SamError> {
    let fields = parse_fields(line);
    if fields.get("RESULT").map(|s| s.as_str()) != Some("OK") {
        return Err(SamError::from_result_line(line));
    }
    if !fields.contains_key("VERSION") {
        return Err(SamError::UnexpectedResponse(line.to_string()));
    }
    Ok(())
}

/// Parses DEST REPLY PUB={pub} PRIV={priv}.
pub fn parse_dest_reply(line: &str) -> Result<(String, String), SamError> {
    let fields = parse_fields(line);
    let pub_key = fields.get("PUB").ok_or_else(|| SamError::UnexpectedResponse(line.to_string()))?;
    let priv_key = fields.get("PRIV").ok_or_else(|| SamError::UnexpectedResponse(line.to_string()))?;
    Ok((pub_key.clone(), priv_key.clone()))
}

/// Parses SESSION STATUS RESULT=OK DESTINATION={dest}.
pub fn parse_session_status(line: &str) -> Result<String, SamError> {
    let fields = parse_fields(line);
    if fields.get("RESULT").map(|s| s.as_str()) != Some("OK") {
        return Err(SamError::from_result_line(line));
    }
    fields.get("DESTINATION")
        .cloned()
        .ok_or_else(|| SamError::UnexpectedResponse(line.to_string()))
}

/// Parses NAMING REPLY RESULT=OK NAME={name} VALUE={dest}.
pub fn parse_naming_reply(line: &str) -> Result<String, SamError> {
    let fields = parse_fields(line);
    if fields.get("RESULT").map(|s| s.as_str()) != Some("OK") {
        return Err(SamError::from_result_line(line));
    }
    fields.get("VALUE")
        .cloned()
        .ok_or_else(|| SamError::UnexpectedResponse(line.to_string()))
}

/// Parses STREAM STATUS RESULT=OK.
pub fn parse_stream_status(line: &str) -> Result<(), SamError> {
    check_result(line)
}

/// Parses the peer address string coming after STREAM STATUS OK upon ACCEPT.
pub fn parse_stream_peer(line: &str) -> Result<String, SamError> {
    if line.trim().is_empty() {
        return Err(SamError::UnexpectedResponse(line.to_string()));
    }
    Ok(line.trim().to_string())
}

/// Parses PONG{payload}.
pub fn parse_pong(line: &str) -> Result<String, SamError> {
    if !line.starts_with("PONG") {
        return Err(SamError::UnexpectedResponse(line.to_string()));
    }
    Ok(line["PONG".len()..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fields_basic() {
        let fields = parse_fields("RESULT=OK VERSION=3.3");
        assert_eq!(fields.get("RESULT").map(|s| s.as_str()), Some("OK"));
        assert_eq!(fields.get("VERSION").map(|s| s.as_str()), Some("3.3"));
    }

    #[test]
    fn parse_fields_ignores_no_eq() {
        let fields = parse_fields("HELLO REPLY RESULT=OK");
        assert_eq!(fields.get("RESULT").map(|s| s.as_str()), Some("OK"));
        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn parse_fields_quoted_spaces() {
        let fields = parse_fields(r#"RESULT=I2P_ERROR MESSAGE="no route""#);
        assert_eq!(fields.get("MESSAGE").map(|s| s.as_str()), Some("no route"));
    }

    #[test]
    fn parse_fields_escaped_quote() {
        let fields = parse_fields(r#"MESSAGE="bad \"key\"""#);
        assert_eq!(fields.get("MESSAGE").map(|s| s.as_str()), Some("bad \"key\""));
    }

    #[test]
    fn check_result_ok() {
        assert!(check_result("RESULT=OK").is_ok());
    }

    #[test]
    fn check_result_err_cant_reach() {
        assert_eq!(check_result("RESULT=CANT_REACH_PEER"), Err(SamError::CantReachPeer));
    }

    #[test]
    fn check_result_err_timeout() {
        assert_eq!(check_result("RESULT=TIMEOUT"), Err(SamError::Timeout));
    }

    #[test]
    fn check_result_err_i2p_error_with_msg() {
        assert_eq!(
            check_result(r#"RESULT=I2P_ERROR MESSAGE="tunnels""#),
            Err(SamError::I2PError("tunnels".into()))
        );
    }

    #[test]
    fn parse_hello_ok() {
        assert!(parse_hello("HELLO REPLY RESULT=OK VERSION=3.3").is_ok());
    }

    #[test]
    fn parse_hello_noversion() {
        assert!(parse_hello("HELLO REPLY RESULT=OK").is_err());
    }

    #[test]
    fn parse_dest_reply_ok() {
        let (pub_key, priv_key) = parse_dest_reply("PUB=abc PRIV=xyz").unwrap();
        assert_eq!(pub_key, "abc");
        assert_eq!(priv_key, "xyz");
    }

    #[test]
    fn parse_dest_reply_missing_field() {
        assert!(parse_dest_reply("PUB=abc").is_err());
    }

    #[test]
    fn parse_session_status_ok() {
        assert_eq!(parse_session_status("DESTINATION=abc RESULT=OK").unwrap(), "abc");
    }

    #[test]
    fn parse_session_status_duplicate_id() {
        assert_eq!(parse_session_status("RESULT=DUPLICATED_ID"), Err(SamError::DuplicatedId));
    }

    #[test]
    fn parse_naming_reply_ok() {
        assert_eq!(parse_naming_reply("RESULT=OK VALUE=dest").unwrap(), "dest");
    }

    #[test]
    fn parse_naming_reply_not_found() {
        assert_eq!(parse_naming_reply("RESULT=KEY_NOT_FOUND"), Err(SamError::KeyNotFound));
    }

    #[test]
    fn parse_stream_status_ok() {
        assert!(parse_stream_status("STREAM STATUS RESULT=OK").is_ok());
    }

    #[test]
    fn parse_stream_status_timeout() {
        assert_eq!(parse_stream_status("STREAM STATUS RESULT=TIMEOUT"), Err(SamError::Timeout));
    }

    #[test]
    fn parse_stream_peer_valid() {
        assert_eq!(parse_stream_peer("abc123").unwrap(), "abc123");
    }

    #[test]
    fn parse_stream_peer_empty() {
        assert!(parse_stream_peer("").is_err());
    }

    #[test]
    fn parse_pong_ok() {
        assert_eq!(parse_pong("PONGhello").unwrap(), "hello");
    }

    #[test]
    fn parse_pong_no_prefix() {
        assert!(parse_pong("something").is_err());
    }
}
