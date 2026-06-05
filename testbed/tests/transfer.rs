use sam3::{ReadyMsg, ResultMsg};
use serde_json::Value;
use std::{
    error::Error,
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use testbed::{
    node::i2pd::{create_multi_reseed_zip, I2pdNode},
    topology::Topology,
};

fn require_root() {
    let uid = unsafe { libc::geteuid() };
    if uid != 0 {
        eprintln!("Tests must be run as root");
        std::process::exit(0);
    }
}

fn check_deps() {
    for dep in ["ip", "i2pd"] {
        which::which(dep).unwrap_or_else(|_| panic!("Dependency {dep} not found"));
    }
}

#[test]
fn test_sam_transfer() -> Result<(), Box<dyn Error>> {
    require_root();
    check_deps();

    let tp = Topology::new(4, "10.88.0");
    let tmp = tempfile::tempdir()?;
    let mut nodes = (0..4)
        .map(|i| {
            I2pdNode::new(
                i,
                tp.nodes[i].ns.clone(),
                tp.nodes[i].ip.clone(),
                12000 + i as u16,
                i == 0,
            )
        })
        .collect::<Vec<_>>();

    eprintln!("Phase 1: generating RouterInfos...");
    nodes[0].write_config(None)?;
    nodes[0].start()?;
    nodes[0].wait_router_info_floodfill(Duration::from_secs(90))?;
    thread::sleep(Duration::from_secs(5));

    let zip_phase1 = tmp.path().join("reseed_phase1.zip");
    nodes[0].create_reseed_zip(&zip_phase1)?;
    for node in nodes.iter().skip(1) {
        node.write_config(Some(&zip_phase1))?;
        node.start()?;
    }
    thread::sleep(Duration::from_secs(15));

    for node in &nodes {
        if !node.router_info_path().exists() {
            return Err(format!("node{} did not generate router.info", node.index).into());
        }
    }

    eprintln!("Phase 1: stopping nodes...");
    for node in &nodes {
        node.stop();
    }
    thread::sleep(Duration::from_secs(3));

    eprintln!("Creating full reseed ZIP...");
    let full_zip = tmp.path().join("reseed_full.zip");
    let refs = nodes.iter().collect::<Vec<_>>();
    create_multi_reseed_zip(&full_zip, &refs)?;

    for node in &nodes {
        let _ = fs::remove_file(node.data_dir.path().join("i2pd.log"));
    }

    eprintln!("Phase 2: starting nodes...");
    nodes[0].write_config(None)?;
    nodes[0].start()?;

    nodes[1].sam_port = Some(7656);
    nodes[1].write_config(Some(&full_zip))?;
    nodes[1].start()?;

    nodes[2].sam_port = Some(7656);
    nodes[2].write_config(Some(&full_zip))?;
    nodes[2].start()?;

    nodes[3].write_config(Some(&full_zip))?;
    nodes[3].start()?;

    eprintln!("Waiting for nodes to bootstrap...");
    for node in nodes.iter().skip(1) {
        node.wait_bootstrapped(Duration::from_secs(240))
            .map_err(|e| {
                print_node_logs(&nodes);
                e
            })?;
    }

    const PAYLOAD_SIZE: i64 = 1024 * 1024;
    const SEED: i64 = 42;
    const TIMEOUT_SECS: &str = "300";
    let run_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

    eprintln!("Starting receiver...");
    let mut recv_cmd = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "sam-receiver",
            "--sam",
            "127.0.0.1:7656",
            "--size",
            &PAYLOAD_SIZE.to_string(),
            "--seed",
            &SEED.to_string(),
            "--id",
            &format!("recv-{run_id}"),
            "--timeout",
            TIMEOUT_SECS,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let recv_stdout = recv_cmd.stdout.take().expect("receiver stdout");
    let recv_stderr = recv_cmd.stderr.take().expect("receiver stderr");
    let (line_tx, line_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        for line in BufReader::new(recv_stdout).lines().map_while(Result::ok) {
            let _ = line_tx.send(line);
        }
    });
    let (stderr_tx, stderr_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut s = String::new();
        let _ = BufReader::new(recv_stderr).read_line(&mut s);
        let _ = stderr_tx.send(s);
    });

    let ready_line = line_rx
        .recv_timeout(Duration::from_secs(120))
        .map_err(|_| {
            eprintln!(
                "sam-receiver stderr:\n{}",
                stderr_rx.try_recv().unwrap_or_default()
            );
            print_node_logs(&nodes);
            "receiver did not send ready message"
        })?;
    let ready: ReadyMsg = serde_json::from_str(&ready_line)?;
    assert_eq!(ready.msg_type, "ready");
    eprintln!(
        "receiver ready, dest={}...",
        &ready.dest[..ready.dest.len().min(20)]
    );

    eprintln!("Starting sender...");
    let sender_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "sam-sender",
            "--sam",
            "127.0.0.1:7656",
            "--dest",
            &ready.dest,
            "--size",
            &PAYLOAD_SIZE.to_string(),
            "--seed",
            &SEED.to_string(),
            "--id",
            &format!("send-{run_id}"),
            "--timeout",
            TIMEOUT_SECS,
        ])
        .output()?;
    if !sender_out.status.success() {
        eprintln!(
            "sam-sender stdout:\n{}",
            String::from_utf8_lossy(&sender_out.stdout)
        );
        eprintln!(
            "sam-sender stderr:\n{}",
            String::from_utf8_lossy(&sender_out.stderr)
        );
        print_connectivity(&tp.nodes[2].ns, &tp.nodes[1].ip);
        print_node_logs(&nodes);
        return Err("sam-sender failed".into());
    }
    let send_result: ResultMsg = serde_json::from_slice(sender_out.stdout.trim_ascii())?;

    let recv_line = line_rx
        .recv_timeout(Duration::from_secs(120))
        .map_err(|_| {
            eprintln!(
                "sam-receiver stderr:\n{}",
                stderr_rx.try_recv().unwrap_or_default()
            );
            print_node_logs(&nodes);
            "receiver did not send result message"
        })?;
    let recv_result: ResultMsg = serde_json::from_str(&recv_line)?;
    let recv_status = recv_cmd.wait()?;

    if !send_result.success || !recv_result.success || !recv_result.sha256_ok {
        eprintln!("sender result: {:?}", as_json(&send_result));
        eprintln!("receiver result: {:?}", as_json(&recv_result));
        print_node_logs(&nodes);
    }

    if recv_result.success {
        assert!(
            recv_status.success(),
            "receiver process failed: {recv_status}"
        );
    }
    assert!(send_result.success, "sender error: {}", send_result.error);
    assert!(recv_result.success, "receiver error: {}", recv_result.error);
    assert!(recv_result.sha256_ok, "sha256 mismatch");
    assert_eq!(recv_result.payload_bytes, PAYLOAD_SIZE);

    eprintln!(
        "setup={:.0}ms transfer={:.0}ms goodput={:.2} Mbps first_byte={:.0}ms",
        send_result.setup_ms,
        send_result.transfer_ms,
        send_result.goodput_mbps,
        recv_result.first_byte_ms
    );
    Ok(())
}

fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn print_connectivity(sender_ns: &str, receiver_ip: &str) {
    let cmd =
        format!("echo x | nc -q1 -w2 {receiver_ip} 12001 && echo REACHABLE || echo UNREACHABLE");
    if let Ok(out) = Command::new("ip")
        .args(["netns", "exec", sender_ns, "bash", "-c", &cmd])
        .output()
    {
        eprintln!(
            "node2->node1 NTCP2 port check: {}",
            String::from_utf8_lossy(&out.stdout).trim()
        );
    }
}

fn print_node_logs(nodes: &[I2pdNode]) {
    let artifact_dir = std::env::var("TEST_ARTIFACT_DIR").ok();
    for node in nodes {
        let log_path = node.data_dir.path().join("i2pd.log");
        let Ok(data) = fs::read_to_string(&log_path) else {
            continue;
        };
        if let Some(dir) = &artifact_dir {
            let dir = Path::new(dir);
            if fs::create_dir_all(dir).is_ok() {
                let dst = dir.join(format!("node{}-i2pd.log", node.index));
                if fs::write(&dst, &data).is_ok() {
                    eprintln!("saved full node{} log to {}", node.index, dst.display());
                }
            }
        }
        let keywords = [
            "SAM",
            "LeaseSet",
            "floodfill",
            "Exploring",
            "routers loaded",
            "Reseed",
            "NTCP2: Connected",
            "NTCP2: Established",
            "transport",
            "error",
        ];
        let lines = data
            .lines()
            .filter(|line| keywords.iter().any(|kw| line.contains(kw)))
            .collect::<Vec<_>>();
        eprintln!(
            "=== node{} relevant log ({} lines) ===\n{}",
            node.index,
            lines.len(),
            lines.join("\n")
        );
    }
}

trait TrimAscii {
    fn trim_ascii(&self) -> &[u8];
}

impl TrimAscii for Vec<u8> {
    fn trim_ascii(&self) -> &[u8] {
        let start = self
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(self.len());
        let end = self
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map(|i| i + 1)
            .unwrap_or(start);
        &self[start..end]
    }
}
