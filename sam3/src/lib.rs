pub mod error;
pub mod proto;
pub mod sync;

pub use error::SamError;
pub use sync::{
    DatagramSession, Destination, ForwardGuard, Incoming, Keys, PrimarySession, PrivateKey, RawSession,
    SamClient, SamConn, SessionOptions, StreamConnectOptions, RawSessionOptions,
    StreamListener, StreamSession,
};

pub const DEFAULT_SIGNATURE_TYPE: &str = crate::proto::command::DEFAULT_SIGNATURE_TYPE;
