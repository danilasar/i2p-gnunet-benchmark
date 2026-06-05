use clap::Parser;
use sam3::{SamClient, SamConn, SamError, SessionOptions, StreamSession};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    role: String,

    #[arg(long, default_value = "127.0.0.1:7656")]
    sam: String,

    #[arg(long, default_value = "rust-compat")]
    id: String,

    #[arg(long)]
    dest: Option<String>,

    #[arg(long)]
    msg: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ReadyMsg {
    #[serde(rename = "type")]
    msg_type: String,
    dest: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ResultMsg {
    #[serde(rename = "type")]
    msg_type: String,
    success: bool,
    error: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    match args.role.as_str() {
        "sender" => run_sender(args),
        "receiver" => run_receiver(args),
        "datagram-sender" => run_datagram_sender(args),
        "datagram-receiver" => run_datagram_receiver(args),
        "raw-sender" => run_raw_sender(args),
        "raw-receiver" => run_raw_receiver(args),
        "primary-sender" => run_primary_sender(args),
        "primary-receiver" => run_primary_receiver(args),
        "lookup" => run_lookup(args),
        _ => Err(format!("Unknown role: {}", args.role).into()),
    }
}

fn run_raw_sender(args: Args) -> Result<(), Box<dyn Error>> {
    let dest = args.dest.ok_or("--dest is required for raw-sender")?;
    let msg = args.msg.ok_or("--msg is required for raw-sender")?;

    let client = SamClient::connect(&args.sam);
    let session = client.new_transient_raw_session(&args.id, &SessionOptions::zero_hop())?;

    session.send_to(msg.as_bytes(), &dest.into())?;

    output_result(true, "");
    Ok(())
}

fn run_raw_receiver(args: Args) -> Result<(), Box<dyn Error>> {
    let msg = args.msg.ok_or("--msg is required for raw-receiver (to verify)")?;

    let client = SamClient::connect(&args.sam);
    let session = client.new_transient_raw_session(&args.id, &SessionOptions::zero_hop())?;
    let dest = session.local_destination().to_string();

    output_ready(&dest);

    session.set_read_timeout(Some(Duration::from_secs(120)))?;
    let mut buf = vec![0u8; 32 * 1024];
    let n = session.read(&mut buf)?;

    if String::from_utf8_lossy(&buf[..n]) != msg {
        return Err(format!(
            "Message mismatch: expected {msg}, got {}",
            String::from_utf8_lossy(&buf[..n])
        )
        .into());
    }

    output_result(true, "");
    Ok(())
}

fn run_datagram_sender(args: Args) -> Result<(), Box<dyn Error>> {
    let dest = args.dest.ok_or("--dest is required for datagram-sender")?;
    let msg = args.msg.ok_or("--msg is required for datagram-sender")?;

    let client = SamClient::connect(&args.sam);
    let session = client.new_transient_datagram_session(&args.id, &SessionOptions::zero_hop())?;

    session.send_to(msg.as_bytes(), &dest.clone().into())?;

    session.set_read_timeout(Some(Duration::from_secs(120)))?;
    let mut buf = vec![0u8; msg.len() + 1024];
    let (n, sender) = session.recv_from(&mut buf)?;

    if sender.as_str() != dest {
        return Err(format!("Sender mismatch: expected {dest}, got {sender}").into());
    }

    if String::from_utf8_lossy(&buf[..n]) != msg {
        return Err(format!("Echo mismatch: expected {msg}, got {}", String::from_utf8_lossy(&buf[..n])).into());
    }

    output_result(true, "");
    Ok(())
}

fn run_datagram_receiver(args: Args) -> Result<(), Box<dyn Error>> {
    let client = SamClient::connect(&args.sam);
    let session = client.new_transient_datagram_session(&args.id, &SessionOptions::zero_hop())?;
    let dest = session.local_destination().to_string();

    output_ready(&dest);

    session.set_read_timeout(Some(Duration::from_secs(120)))?;
    let mut buf = vec![0u8; 32 * 1024];
    let (n, sender) = session.recv_from(&mut buf)?;

    session.send_to(&buf[..n], &sender)?;

    output_result(true, "");
    Ok(())
}

fn run_sender(args: Args) -> Result<(), Box<dyn Error>> {
    let dest = args.dest.ok_or("--dest is required for sender")?;
    let msg = args.msg.ok_or("--msg is required for sender")?;

    let client = SamClient::connect(&args.sam);
    let session = client.new_transient_stream_session(&args.id, &SessionOptions::zero_hop())?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut conn = dial_with_retry(&session, &dest, deadline)?;

    conn.write_all(msg.as_bytes())?;
    conn.flush()?;

    let mut buf = vec![0u8; msg.len()];
    conn.read_exact(&mut buf)?;

    if String::from_utf8_lossy(&buf) != msg {
        return Err(format!("Echo mismatch: expected {msg}, got {}", String::from_utf8_lossy(&buf)).into());
    }

    output_result(true, "");
    Ok(())
}

fn dial_with_retry(session: &StreamSession, dest: &str, deadline: Instant) -> Result<SamConn, Box<dyn Error>> {
    let per_attempt = Duration::from_secs(90);
    let retry_interval = Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("dial deadline exceeded".into());
        }
        match session.dial_timeout(dest, per_attempt.min(remaining)) {
            Ok(conn) => return Ok(conn),
            Err(SamError::CantReachPeer | SamError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
        if Instant::now() + retry_interval > deadline {
            return Err("dial deadline exceeded after retry interval".into());
        }
        thread::sleep(retry_interval);
    }
}

fn run_receiver(args: Args) -> Result<(), Box<dyn Error>> {
    let client = SamClient::connect(&args.sam);
    let session = client.new_transient_stream_session(&args.id, &SessionOptions::zero_hop())?;
    let dest = session.destination().to_string();
    let listener = session.listen();

    output_ready(&dest);

    let mut conn = listener.accept_timeout(Duration::from_secs(120))?;

    if let Some(ref msg) = args.msg {
        // Known message length — deterministic read_exact + echo, no timeout heuristics
        let mut buf = vec![0u8; msg.len()];
        conn.read_exact(&mut buf)?;
        conn.write_all(&buf)?;
        conn.flush()?;
    } else {
        // Unknown length — timeout-based echo loop (fallback for ad-hoc use)
        conn.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut tmp = [0u8; 1024];
        loop {
            match conn.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    conn.write_all(&tmp[..n])?;
                    conn.flush()?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }
        // Grace period: let the remote read the echoed data before closing
        thread::sleep(Duration::from_secs(1));
    }

    output_result(true, "");
    Ok(())
}

fn run_primary_sender(args: Args) -> Result<(), Box<dyn Error>> {
    let dest = args.dest.ok_or("--dest is required for primary-sender")?;
    let msg = args.msg.ok_or("--msg is required for primary-sender")?;

    let client = SamClient::connect(&args.sam);
    let mut primary = client.new_transient_primary_session(&args.id, &SessionOptions::zero_hop())?;
    let sub = primary.new_stream_sub_session(format!("{}-sub", args.id))?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut conn = dial_with_retry(&sub, &dest, deadline)?;

    conn.write_all(msg.as_bytes())?;
    conn.flush()?;

    let mut buf = vec![0u8; msg.len()];
    conn.read_exact(&mut buf)?;

    if String::from_utf8_lossy(&buf) != msg {
        return Err(format!("Echo mismatch: expected {msg}, got {}", String::from_utf8_lossy(&buf)).into());
    }

    output_result(true, "");
    Ok(())
}

fn run_primary_receiver(args: Args) -> Result<(), Box<dyn Error>> {
    let client = SamClient::connect(&args.sam);
    let mut primary = client.new_transient_primary_session(&args.id, &SessionOptions::zero_hop())?;
    let sub = primary.new_stream_sub_session(format!("{}-sub", args.id))?;
    let dest = sub.destination().to_string();
    let listener = sub.listen();

    output_ready(&dest);

    let mut conn = listener.accept_timeout(Duration::from_secs(120))?;

    if let Some(ref msg) = args.msg {
        // Known message length — deterministic read_exact + echo, no timeout heuristics
        let mut buf = vec![0u8; msg.len()];
        conn.read_exact(&mut buf)?;
        conn.write_all(&buf)?;
        conn.flush()?;
    } else {
        // Unknown length — timeout-based echo loop (fallback for ad-hoc use)
        conn.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut tmp = [0u8; 1024];
        loop {
            match conn.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    conn.write_all(&tmp[..n])?;
                    conn.flush()?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }
        // Grace period: let the remote read the echoed data before closing
        thread::sleep(Duration::from_secs(1));
    }

    output_result(true, "");
    Ok(())
}

fn run_lookup(args: Args) -> Result<(), Box<dyn Error>> {
    let dest = args.dest.ok_or("--dest is required for lookup")?;
    let client = SamClient::connect(&args.sam);
    let resolved = client.lookup(&dest)?;

    output_ready(resolved.as_str());
    output_result(true, "");
    Ok(())
}

fn output_ready(dest: &str) {
    let msg = ReadyMsg {
        msg_type: "ready".to_string(),
        dest: dest.to_string(),
    };
    println!("{}", serde_json::to_string(&msg).unwrap());
}

fn output_result(success: bool, err: &str) {
    let msg = ResultMsg {
        msg_type: "result".to_string(),
        success,
        error: err.to_string(),
    };
    println!("{}", serde_json::to_string(&msg).unwrap());
}
