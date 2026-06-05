use crate::error::SamError;
use crate::proto::response;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Fresh,
    HelloPending,
    HelloDone,
    CreatePending,
    Active(String),    // destination
    Poisoned(String),  // error description
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum StreamOp { Connect, Accept, Forward }

#[derive(Debug, Clone, PartialEq)]
pub enum StreamOpState {
    Fresh,
    HelloPending,
    HelloDone,
    OpPending(StreamOp),
    Done,
    Poisoned(String),
}

pub struct SessionController {
    state: SessionState,
}

pub struct StreamOpController {
    state: StreamOpState,
}

impl SessionController {
    pub fn new() -> Self {
        Self { state: SessionState::Fresh }
    }

    pub fn begin_handshake(&mut self) -> Result<(), SamError> {
        match &self.state {
            SessionState::Fresh => {
                self.state = SessionState::HelloPending;
                Ok(())
            }
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition to HelloPending from {:?}", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn on_hello_reply(&mut self, line: &str) -> Result<(), SamError> {
        match &self.state {
            SessionState::HelloPending => {
                match response::parse_hello(line) {
                    Ok(()) => {
                        self.state = SessionState::HelloDone;
                        Ok(())
                    }
                    Err(e) => {
                        self.state = SessionState::Poisoned(e.to_string());
                        Err(e)
                    }
                }
            }
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition from {:?} on hello reply", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn begin_create(&mut self) -> Result<(), SamError> {
        match &self.state {
            SessionState::HelloDone => {
                self.state = SessionState::CreatePending;
                Ok(())
            }
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition to CreatePending from {:?}", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn on_session_status(&mut self, line: &str) -> Result<String, SamError> {
        match &self.state {
            SessionState::CreatePending => {
                match response::parse_session_status(line) {
                    Ok(dest) => {
                        self.state = SessionState::Active(dest.clone());
                        Ok(dest)
                    }
                    Err(e) => {
                        self.state = SessionState::Poisoned(e.to_string());
                        Err(e)
                    }
                }
            }
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition from {:?} on session status", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn begin_session_add(&mut self) -> Result<(), SamError> {
        match &self.state {
            SessionState::Active(_) => Ok(()),
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("cannot add subsession in state {:?}", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn on_session_add_reply(&mut self, line: &str) -> Result<(), SamError> {
        match &self.state {
            SessionState::Active(_) => response::check_result(line),
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("unexpected session add reply in state {:?}", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn begin_session_remove(&mut self) -> Result<(), SamError> {
        match &self.state {
            SessionState::Active(_) => Ok(()),
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("cannot remove subsession in state {:?}", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn on_session_remove_reply(&mut self, line: &str) -> Result<(), SamError> {
        match &self.state {
            SessionState::Active(_) => response::check_result(line),
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("unexpected session remove reply in state {:?}", self.state);
                self.state = SessionState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn destination(&self) -> Result<&str, SamError> {
        match &self.state {
            SessionState::Active(dest) => Ok(dest),
            SessionState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => Err(SamError::UnexpectedResponse(format!("session not active, state: {:?}", self.state))),
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }
}

impl StreamOpController {
    pub fn new() -> Self {
        Self { state: StreamOpState::Fresh }
    }

    pub fn begin_handshake(&mut self) -> Result<(), SamError> {
        match &self.state {
            StreamOpState::Fresh => {
                self.state = StreamOpState::HelloPending;
                Ok(())
            }
            StreamOpState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition to HelloPending from {:?}", self.state);
                self.state = StreamOpState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn on_hello_reply(&mut self, line: &str) -> Result<(), SamError> {
        match &self.state {
            StreamOpState::HelloPending => {
                match response::parse_hello(line) {
                    Ok(()) => {
                        self.state = StreamOpState::HelloDone;
                        Ok(())
                    }
                    Err(e) => {
                        self.state = StreamOpState::Poisoned(e.to_string());
                        Err(e)
                    }
                }
            }
            StreamOpState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition from {:?} on hello reply", self.state);
                self.state = StreamOpState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn begin_op(&mut self, op: StreamOp) -> Result<(), SamError> {
        match &self.state {
            StreamOpState::HelloDone => {
                self.state = StreamOpState::OpPending(op);
                Ok(())
            }
            StreamOpState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition to OpPending from {:?}", self.state);
                self.state = StreamOpState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn on_stream_status(&mut self, line: &str) -> Result<(), SamError> {
        match &self.state {
            StreamOpState::OpPending(_) => {
                match response::parse_stream_status(line) {
                    Ok(()) => {
                        self.state = StreamOpState::Done;
                        Ok(())
                    }
                    Err(e) => {
                        self.state = StreamOpState::Poisoned(e.to_string());
                        Err(e)
                    }
                }
            }
            StreamOpState::Poisoned(msg) => Err(SamError::Poisoned(msg.clone())),
            _ => {
                let msg = format!("invalid transition from {:?} on stream status", self.state);
                self.state = StreamOpState::Poisoned(msg.clone());
                Err(SamError::Poisoned(msg))
            }
        }
    }

    pub fn state(&self) -> &StreamOpState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_happy_path() {
        let mut ctrl = SessionController::new();
        assert_eq!(ctrl.state(), &SessionState::Fresh);

        ctrl.begin_handshake().unwrap();
        assert_eq!(ctrl.state(), &SessionState::HelloPending);

        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        assert_eq!(ctrl.state(), &SessionState::HelloDone);

        ctrl.begin_create().unwrap();
        assert_eq!(ctrl.state(), &SessionState::CreatePending);

        let dest = ctrl.on_session_status("SESSION STATUS RESULT=OK DESTINATION=abcd").unwrap();
        assert_eq!(dest, "abcd");
        assert_eq!(ctrl.state(), &SessionState::Active("abcd".to_string()));
    }

    #[test]
    fn session_double_begin_handshake() {
        let mut ctrl = SessionController::new();
        ctrl.begin_handshake().unwrap();
        let res = ctrl.begin_handshake();
        assert!(matches!(res, Err(SamError::Poisoned(_))));
        assert!(matches!(ctrl.state(), SessionState::Poisoned(_)));
    }

    #[test]
    fn session_skip_hello() {
        let mut ctrl = SessionController::new();
        let res = ctrl.begin_create();
        assert!(matches!(res, Err(SamError::Poisoned(_))));
        assert!(matches!(ctrl.state(), SessionState::Poisoned(_)));
    }

    #[test]
    fn session_poisoned_stays_poisoned() {
        let mut ctrl = SessionController::new();
        ctrl.begin_create().unwrap_err();
        let res = ctrl.begin_handshake();
        assert!(matches!(res, Err(SamError::Poisoned(_))));
    }

    #[test]
    fn session_add_on_active() {
        let mut ctrl = SessionController::new();
        ctrl.begin_handshake().unwrap();
        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        ctrl.begin_create().unwrap();
        ctrl.on_session_status("SESSION STATUS RESULT=OK DESTINATION=abcd").unwrap();

        ctrl.begin_session_add().unwrap();
        ctrl.on_session_add_reply("SESSION STATUS RESULT=OK").unwrap();
        assert_eq!(ctrl.state(), &SessionState::Active("abcd".to_string()));
    }

    #[test]
    fn session_add_on_fresh_poisons() {
        let mut ctrl = SessionController::new();
        let res = ctrl.begin_session_add();
        assert!(matches!(res, Err(SamError::Poisoned(_))));
        assert!(matches!(ctrl.state(), SessionState::Poisoned(_)));
    }

    #[test]
    fn session_remove_on_fresh_poisons() {
        let mut ctrl = SessionController::new();
        let res = ctrl.begin_session_remove();
        assert!(matches!(res, Err(SamError::Poisoned(_))));
        assert!(matches!(ctrl.state(), SessionState::Poisoned(_)));
    }

    #[test]
    fn session_destination_when_active() {
        let mut ctrl = SessionController::new();
        ctrl.begin_handshake().unwrap();
        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        ctrl.begin_create().unwrap();
        ctrl.on_session_status("SESSION STATUS RESULT=OK DESTINATION=abcd").unwrap();

        assert_eq!(ctrl.destination().unwrap(), "abcd");
    }

    #[test]
    fn session_destination_when_not_active() {
        let ctrl = SessionController::new();
        assert!(ctrl.destination().is_err());
    }

    #[test]
    fn stream_op_happy_connect() {
        let mut ctrl = StreamOpController::new();
        ctrl.begin_handshake().unwrap();
        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        ctrl.begin_op(StreamOp::Connect).unwrap();
        ctrl.on_stream_status("STREAM STATUS RESULT=OK").unwrap();
        assert_eq!(ctrl.state(), &StreamOpState::Done);
    }

    #[test]
    fn stream_op_happy_accept() {
        let mut ctrl = StreamOpController::new();
        ctrl.begin_handshake().unwrap();
        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        ctrl.begin_op(StreamOp::Accept).unwrap();
        ctrl.on_stream_status("STREAM STATUS RESULT=OK").unwrap();
        assert_eq!(ctrl.state(), &StreamOpState::Done);
    }

    #[test]
    fn stream_op_happy_forward() {
        let mut ctrl = StreamOpController::new();
        ctrl.begin_handshake().unwrap();
        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        ctrl.begin_op(StreamOp::Forward).unwrap();
        ctrl.on_stream_status("STREAM STATUS RESULT=OK").unwrap();
        assert_eq!(ctrl.state(), &StreamOpState::Done);
    }

    #[test]
    fn stream_op_begin_op_without_hello() {
        let mut ctrl = StreamOpController::new();
        let res = ctrl.begin_op(StreamOp::Connect);
        assert!(matches!(res, Err(SamError::Poisoned(_))));
        assert!(matches!(ctrl.state(), StreamOpState::Poisoned(_)));
    }

    #[test]
    fn stream_op_double_begin_op() {
        let mut ctrl = StreamOpController::new();
        ctrl.begin_handshake().unwrap();
        ctrl.on_hello_reply("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        ctrl.begin_op(StreamOp::Connect).unwrap();
        let res = ctrl.begin_op(StreamOp::Connect);
        assert!(matches!(res, Err(SamError::Poisoned(_))));
    }

    #[test]
    fn stream_op_poisoned_blocks() {
        let mut ctrl = StreamOpController::new();
        ctrl.begin_op(StreamOp::Connect).unwrap_err();
        let res = ctrl.begin_handshake();
        assert!(matches!(res, Err(SamError::Poisoned(_))));
    }
}
