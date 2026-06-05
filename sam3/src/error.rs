use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug, PartialEq, Eq)]
pub enum SamError {
    DuplicatedDest,
    DuplicatedId,
    CantReachPeer,
    KeyNotFound,
    InvalidKey,
    InvalidId,
    Timeout,
    SessionNotFound,
    NotASession,
    I2PError(String),
    UnexpectedResponse(String),
    Io(String),
}

impl SamError {
    pub(crate) fn from_result_line(line: &str) -> Self {
        let fields = crate::session::parse_fields(line);
        let Some(result) = fields.get("RESULT") else {
            return SamError::UnexpectedResponse(line.to_string());
        };

        match result.as_str() {
            "DUPLICATED_DEST" => SamError::DuplicatedDest,
            "DUPLICATED_ID" => SamError::DuplicatedId,
            "CANT_REACH_PEER" => SamError::CantReachPeer,
            "KEY_NOT_FOUND" => SamError::KeyNotFound,
            "INVALID_KEY" => SamError::InvalidKey,
            "INVALID_ID" => SamError::InvalidId,
            "TIMEOUT" => SamError::Timeout,
            "SESSION_NOT_FOUND" => SamError::SessionNotFound,
            "NOTSUPPORTED" => SamError::NotASession,
            "I2P_ERROR" => SamError::I2PError(fields.get("MESSAGE").cloned().unwrap_or_default()),
            _ => SamError::UnexpectedResponse(line.to_string()),
        }
    }
}

impl fmt::Display for SamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SamError::DuplicatedDest => f.write_str("DUPLICATED_DEST"),
            SamError::DuplicatedId => f.write_str("DUPLICATED_ID"),
            SamError::CantReachPeer => f.write_str("CANT_REACH_PEER"),
            SamError::KeyNotFound => f.write_str("KEY_NOT_FOUND"),
            SamError::InvalidKey => f.write_str("INVALID_KEY"),
            SamError::InvalidId => f.write_str("INVALID_ID"),
            SamError::Timeout => f.write_str("TIMEOUT"),
            SamError::SessionNotFound => f.write_str("SESSION_NOT_FOUND"),
            SamError::NotASession => f.write_str("NOTSUPPORTED"),
            SamError::I2PError(msg) if msg.is_empty() => f.write_str("I2P_ERROR"),
            SamError::I2PError(msg) => write!(f, "I2P_ERROR: {msg}"),
            SamError::UnexpectedResponse(line) => write!(f, "unexpected SAM response: {line}"),
            SamError::Io(msg) => write!(f, "IO error: {msg}"),
        }
    }
}

impl Error for SamError {}

impl From<io::Error> for SamError {
    fn from(err: io::Error) -> Self {
        SamError::Io(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_result_line_parses_cant_reach_peer() {
        assert_eq!(
            SamError::from_result_line("STREAM STATUS RESULT=CANT_REACH_PEER"),
            SamError::CantReachPeer
        );
    }

    #[test]
    fn from_result_line_parses_i2p_error_with_message() {
        assert_eq!(
            SamError::from_result_line(r#"STREAM STATUS RESULT=I2P_ERROR MESSAGE="no tunnels""#),
            SamError::I2PError("no tunnels".into())
        );
    }

    #[test]
    fn from_result_line_parses_i2p_error_without_message() {
        assert_eq!(
            SamError::from_result_line("DEST REPLY RESULT=I2P_ERROR"),
            SamError::I2PError(String::new())
        );
    }

    #[test]
    fn from_result_line_unknown_result_becomes_unexpected() {
        let e = SamError::from_result_line("STREAM STATUS RESULT=SOMETHING_NEW");
        assert!(matches!(e, SamError::UnexpectedResponse(_)));
    }

    #[test]
    fn from_result_line_missing_result_becomes_unexpected() {
        let e = SamError::from_result_line("STREAM STATUS");
        assert!(matches!(e, SamError::UnexpectedResponse(_)));
    }

    #[test]
    fn from_result_line_parses_duplicated_id() {
        assert_eq!(
            SamError::from_result_line("SESSION STATUS RESULT=DUPLICATED_ID"),
            SamError::DuplicatedId
        );
    }

    #[test]
    fn display_uses_protocol_codes() {
        assert_eq!(format!("{}", SamError::CantReachPeer), "CANT_REACH_PEER");
        assert_eq!(format!("{}", SamError::I2PError("no tunnels".into())), "I2P_ERROR: no tunnels");
        assert_eq!(format!("{}", SamError::I2PError(String::new())), "I2P_ERROR");
        assert!(format!("{}", SamError::UnexpectedResponse("bad line".into())).contains("bad line"));
    }
}
