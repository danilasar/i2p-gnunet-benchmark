use super::{messages::ResultMsg, payload::PayloadReader, wire::send_payload};
use sam3::{SamClient, SamConn, SamError, StreamSession, SAM_TUNNEL_OPTIONS};
use std::{
    error::Error,
    io::Write,
    thread,
    time::{Duration, Instant},
};

pub struct SenderConfig {
    pub sam_addr: String,
    pub dest: String,
    pub size: i64,
    pub seed: i64,
    pub id: String,
    pub timeout: Duration,
}

pub fn run_sender(cfg: SenderConfig, w: &mut impl Write) -> Result<(), Box<dyn Error>> {
    let mut res = ResultMsg::new("sender");
    let fail =
        |res: &mut ResultMsg, w: &mut dyn Write, msg: String| -> Result<(), Box<dyn Error>> {
            res.error = msg.clone();
            serde_json::to_writer(&mut *w, res)?;
            writeln!(w)?;
            Err(msg.into())
        };

    let t0 = Instant::now();
    let deadline = t0 + cfg.timeout;
    let client = SamClient::connect(&cfg.sam_addr);
    let session = match client.new_transient_stream_session(&cfg.id, SAM_TUNNEL_OPTIONS) {
        Ok(s) => s,
        Err(e) => return fail(&mut res, w, format!("SAM session: {e}")),
    };

    let mut conn = match dial_with_retry(&session, cfg.dest.clone(), deadline) {
        Ok(c) => c,
        Err(e) => return fail(&mut res, w, format!("DialI2P: {e}")),
    };

    let t1 = Instant::now();
    res.setup_ms = t1.duration_since(t0).as_millis() as f64;

    let mut reader = PayloadReader::new(cfg.size, cfg.seed);
    let (written, _) = match send_payload(&mut conn, &mut reader, cfg.size) {
        Ok(v) => v,
        Err(e) => return fail(&mut res, w, format!("SendPayload: {e}")),
    };
    let t2 = Instant::now();
    thread::sleep(Duration::from_secs(5));
    drop(conn);
    drop(session);

    res.payload_bytes = written;
    res.success = true;
    res.sha256_ok = true;
    res.transfer_ms = t2.duration_since(t1).as_millis() as f64;
    if res.transfer_ms > 0.0 {
        res.goodput_mbps = (written as f64 * 8.0 / 1e6) / (res.transfer_ms / 1000.0);
    }
    serde_json::to_writer(&mut *w, &res)?;
    writeln!(w)?;
    Ok(())
}

fn dial_with_retry(
    session: &StreamSession,
    dest: String,
    deadline: Instant,
) -> Result<SamConn, SamError> {
    let per_attempt = Duration::from_secs(90);
    let retry_interval = Duration::from_secs(5);

    loop {
        let attempt_timeout = per_attempt.min(deadline.saturating_duration_since(Instant::now()));
        let last_err = match session.dial_timeout(&dest, attempt_timeout) {
            Ok(conn) => return Ok(conn),
            Err(e) => e,
        };

        if Instant::now() + retry_interval > deadline {
            return Err(last_err);
        }
        thread::sleep(retry_interval);
    }
}
