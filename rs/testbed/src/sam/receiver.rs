use super::{
    messages::{ReadyMsg, ResultMsg},
    wire::receive_payload,
};
use std::{
    error::Error,
    io::Write,
    thread,
    time::{Duration, Instant},
};

use super::sam3::{SamSession, SAM_TUNNEL_OPTIONS};

pub struct ReceiverConfig {
    pub sam_addr: String,
    pub size: i64,
    pub seed: i64,
    pub id: String,
    pub timeout: Duration,
}

pub fn run_receiver(cfg: ReceiverConfig, w: &mut impl Write) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + cfg.timeout;
    let mut res = ResultMsg::new("receiver");
    let fail =
        |res: &mut ResultMsg, w: &mut dyn Write, msg: String| -> Result<(), Box<dyn Error>> {
            res.error = msg.clone();
            serde_json::to_writer(&mut *w, res)?;
            writeln!(w)?;
            Err(msg.into())
        };

    let session = match SamSession::create_stream(&cfg.sam_addr, &cfg.id, SAM_TUNNEL_OPTIONS) {
        Ok(s) => s,
        Err(e) => return fail(&mut res, w, format!("SAM session: {e}")),
    };

    serde_json::to_writer(&mut *w, &ReadyMsg::new(session.destination.clone()))?;
    writeln!(w)?;
    w.flush()?;

    loop {
        if Instant::now() >= deadline {
            return fail(&mut res, w, "timeout waiting for connection".to_string());
        }

        let t_accept = Instant::now();
        let mut conn = match SamSession::accept_stream(&cfg.sam_addr, &cfg.id) {
            Ok(c) => c,
            Err(e) => return fail(&mut res, w, format!("Accept: {e}")),
        };

        let result = receive_payload(&mut conn);
        let t_done = Instant::now();
        drop(conn);

        match result {
            Ok((received, sha_ok, first_byte_at)) => {
                res.payload_bytes = received;
                res.sha256_ok = sha_ok;
                res.success = true;
                res.first_byte_ms = first_byte_at.duration_since(t_accept).as_millis() as f64;
                res.transfer_ms = t_done.duration_since(first_byte_at).as_millis() as f64;
                drop(session);
                serde_json::to_writer(&mut *w, &res)?;
                writeln!(w)?;
                return Ok(());
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("failed to read size:") && msg.contains("EOF") {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                res.payload_bytes = 0;
                return fail(&mut res, w, format!("ReceivePayload: {msg}"));
            }
        }
    }
}
