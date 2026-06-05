use serde::Deserialize;
use std::{
    error::Error,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use testbed::{
    node::i2pd::{create_multi_reseed_zip, I2pdNode},
    topology::Topology,
};

#[derive(Debug, Deserialize)]
struct GoReadyMsg {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    msg_type: String,
    dest: String,
}

#[derive(Debug, Deserialize)]
struct GoResultMsg {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    msg_type: String,
    success: bool,
    error: String,
}

fn require_root() {
    let uid = unsafe { libc::geteuid() };
    if uid != 0 {
        eprintln!("Tests must be run as root");
        std::process::exit(0);
    }
}

fn setup_two_sam_nodes() -> Result<(Topology, Vec<I2pdNode>, tempfile::TempDir), Box<dyn Error>> {
    require_root();
    let tp = Topology::new(3, "10.89.0");
    let tmp = tempfile::tempdir()?;
    let mut nodes = (0..3)
        .map(|i| {
            I2pdNode::new(
                i,
                tp.nodes[i].ns.clone(),
                tp.nodes[i].ip.clone(),
                13000 + i as u16,
                i == 0,
            )
        })
        .collect::<Vec<_>>();

    nodes[0].write_config(None)?;
    nodes[0].start()?;
    nodes[0].wait_router_info_floodfill(Duration::from_secs(90))?;

    let zip_phase1 = tmp.path().join("reseed_phase1.zip");
    nodes[0].create_reseed_zip(&zip_phase1)?;
    for node in nodes.iter().skip(1) {
        node.write_config(Some(&zip_phase1))?;
        node.start()?;
    }
    thread::sleep(Duration::from_secs(15));

    for node in &nodes {
        node.stop();
    }
    thread::sleep(Duration::from_secs(3));

    let full_zip = tmp.path().join("reseed_full.zip");
    let refs = nodes.iter().collect::<Vec<_>>();
    create_multi_reseed_zip(&full_zip, &refs)?;

    nodes[0].write_config(None)?;
    nodes[0].start()?;

    for node in nodes.iter_mut().skip(1) {
        node.sam_port = Some(7656);
        node.write_config(Some(&full_zip))?;
        node.start()?;
    }

    for node in nodes.iter().skip(1) {
        node.wait_bootstrapped(Duration::from_secs(180))?;
    }

    Ok((tp, nodes, tmp))
}

#[test]
fn test_rust_sender_go_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;

    // 1. Start Go server in node1
    let mut go_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "go-sam3-peer",
            "--role",
            "server",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-server",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut go_stdout = BufReader::new(go_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    go_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let go_dest = ready.dest;

    // 2. Start Rust client in node2 via netns
    let msg = "hello from rust";
    let rust_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "sam-compat",
            "--role",
            "sender",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-client",
            "--dest",
            &go_dest,
            "--msg",
            msg,
        ])
        .output()?;

    if !rust_out.status.success() {
        eprintln!("rust-sender failed: {}", String::from_utf8_lossy(&rust_out.stderr));
    }
    let rust_result: GoResultMsg = serde_json::from_slice(&rust_out.stdout)?;
    assert!(rust_result.success, "Rust sender failed: {}", rust_result.error);

    // 3. Check Go result
    let mut result_line = String::new();
    go_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Go server failed: {}", result.error);

    go_proc.wait()?;
    Ok(())
}

#[test]
fn test_go_sender_rust_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello from go";

    // 1. Start Rust receiver in node1 via netns
    let mut rust_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "sam-compat",
            "--role",
            "receiver",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-server",
            "--msg",
            msg,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut rust_stdout = BufReader::new(rust_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    rust_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let rust_dest = ready.dest;

    // 2. Start Go client in node2
    let go_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "go-sam3-peer",
            "--role",
            "client",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-client",
            "--dest",
            &rust_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let lines: Vec<_> = go_out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty()).collect();
    let result: GoResultMsg = serde_json::from_slice(lines.last().ok_or("no output from go-sam3-peer")?)?;
    assert!(result.success, "Go client failed: {}", result.error);

    // 3. Check Rust result
    let mut result_line = String::new();
    rust_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Rust receiver failed: {}", result.error);

    rust_proc.wait()?;
    Ok(())
}

#[test]
fn test_rust_datagram_sender_go_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello datagram from rust";

    // 1. Start Go datagram-server in node1
    let mut go_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "go-sam3-peer",
            "--role",
            "datagram-server",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-dg-server",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut go_stdout = BufReader::new(go_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    go_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let go_dest = ready.dest;

    // 2. Start Rust datagram-sender in node2
    let rust_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "sam-compat",
            "--role",
            "datagram-sender",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-dg-client",
            "--dest",
            &go_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let rust_result: GoResultMsg = serde_json::from_slice(&rust_out.stdout)?;
    assert!(rust_result.success, "Rust datagram sender failed: {}", rust_result.error);

    // 3. Check Go result
    let mut result_line = String::new();
    go_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Go datagram server failed: {}", result.error);

    go_proc.wait()?;
    Ok(())
}

#[test]
fn test_go_datagram_sender_rust_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello datagram from go";

    // 1. Start Rust datagram-receiver in node1
    let mut rust_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "sam-compat",
            "--role",
            "datagram-receiver",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-dg-receiver",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut rust_stdout = BufReader::new(rust_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    rust_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let rust_dest = ready.dest;

    // 2. Start Go datagram-client in node2
    let go_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "go-sam3-peer",
            "--role",
            "datagram-client",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-dg-client",
            "--dest",
            &rust_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let lines: Vec<_> = go_out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty()).collect();
    let result: GoResultMsg = serde_json::from_slice(lines.last().ok_or("no output from go-sam3-peer")?)?;
    assert!(result.success, "Go datagram client failed: {}", result.error);

    // 3. Check Rust result
    let mut result_line = String::new();
    rust_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Rust datagram receiver failed: {}", result.error);

    rust_proc.wait()?;
    Ok(())
}

#[test]
fn test_rust_raw_sender_go_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello raw from rust";

    // 1. Start Go raw-server in node1
    let mut go_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "go-sam3-peer",
            "--role",
            "raw-server",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-raw-server",
            "--msg",
            msg,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut go_stdout = BufReader::new(go_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    go_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let go_dest = ready.dest;

    // 2. Start Rust raw-sender in node2
    let rust_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "sam-compat",
            "--role",
            "raw-sender",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-raw-sender",
            "--dest",
            &go_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let rust_result: GoResultMsg = serde_json::from_slice(&rust_out.stdout)?;
    assert!(
        rust_result.success,
        "Rust raw sender failed: {}",
        rust_result.error
    );

    // 3. Check Go result
    let mut result_line = String::new();
    go_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Go raw server failed: {}", result.error);

    go_proc.wait()?;
    Ok(())
}

#[test]
fn test_go_raw_sender_rust_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello raw from go";

    // 1. Start Rust raw-receiver in node1
    let mut rust_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "sam-compat",
            "--role",
            "raw-receiver",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-raw-receiver",
            "--msg",
            msg,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut rust_stdout = BufReader::new(rust_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    rust_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let rust_dest = ready.dest;

    // 2. Start Go raw-client in node2
    let go_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "go-sam3-peer",
            "--role",
            "raw-client",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-raw-client",
            "--dest",
            &rust_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let lines: Vec<_> = go_out
        .stdout
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    let result: GoResultMsg =
        serde_json::from_slice(lines.last().ok_or("no output from go-sam3-peer")?)?;
    assert!(result.success, "Go raw client failed: {}", result.error);

    // 3. Check Rust result
    let mut result_line = String::new();
    rust_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Rust raw receiver failed: {}", result.error);

    rust_proc.wait()?;
    Ok(())
}

#[test]
fn test_rust_primary_sender_go_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello primary from rust";

    // 1. Start Go primary-server in node1
    let mut go_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "go-sam3-peer",
            "--role",
            "primary-server",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-prim-server",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut go_stdout = BufReader::new(go_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    go_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let go_dest = ready.dest;

    // 2. Start Rust primary-sender in node2
    let rust_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "sam-compat",
            "--role",
            "primary-sender",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-prim-sender",
            "--dest",
            &go_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let rust_result: GoResultMsg = serde_json::from_slice(&rust_out.stdout)?;
    assert!(
        rust_result.success,
        "Rust primary sender failed: {}",
        rust_result.error
    );

    // 3. Check Go result
    let mut result_line = String::new();
    go_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Go primary server failed: {}", result.error);

    go_proc.wait()?;
    Ok(())
}

#[test]
fn test_go_primary_sender_rust_receiver() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;
    let msg = "hello primary from go";

    // 1. Start Rust primary-receiver in node1
    let mut rust_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "sam-compat",
            "--role",
            "primary-receiver",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "rust-prim-receiver",
            "--msg",
            msg,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut rust_stdout = BufReader::new(rust_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    rust_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let rust_dest = ready.dest;

    // 2. Start Go primary-client in node2
    let go_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "go-sam3-peer",
            "--role",
            "primary-client",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-prim-client",
            "--dest",
            &rust_dest,
            "--msg",
            msg,
        ])
        .output()?;

    let lines: Vec<_> = go_out
        .stdout
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    let result: GoResultMsg =
        serde_json::from_slice(lines.last().ok_or("no output from go-sam3-peer")?)?;
    assert!(result.success, "Go primary client failed: {}", result.error);

    // 3. Check Rust result
    let mut result_line = String::new();
    rust_stdout.read_line(&mut result_line)?;
    let result: GoResultMsg = serde_json::from_str(&result_line)?;
    assert!(result.success, "Rust primary receiver failed: {}", result.error);

    rust_proc.wait()?;
    Ok(())
}

#[test]
fn test_lookup_against_go_destination() -> Result<(), Box<dyn Error>> {
    let (tp, _nodes, _tmp) = setup_two_sam_nodes()?;

    // 1. Start Go server in node1
    let mut go_proc = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[1].ns,
            "go-sam3-peer",
            "--role",
            "server",
            "--sam",
            "127.0.0.1:7656",
            "--id",
            "go-lookup-target",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut go_stdout = BufReader::new(go_proc.stdout.take().unwrap());
    let mut ready_line = String::new();
    go_stdout.read_line(&mut ready_line)?;
    let ready: GoReadyMsg = serde_json::from_str(&ready_line)?;
    let go_dest = ready.dest;

    // 2. Wait for Go server to publish its LeaseSet
    thread::sleep(Duration::from_secs(25));
    
    // 3. Resolve b32 via Rust in node2
    let go_b32 = derive_b32(&go_dest);
    let rust_out = Command::new("ip")
        .args([
            "netns",
            "exec",
            &tp.nodes[2].ns,
            "sam-compat",
            "--role",
            "lookup",
            "--sam",
            "127.0.0.1:7656",
            "--dest",
            &go_b32,
        ])
        .output()?;
    
    let lines: Vec<_> = rust_out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty()).collect();
    let res_ready: GoReadyMsg = serde_json::from_slice(lines[0])?;
    assert_eq!(res_ready.dest, go_dest);

    go_proc.kill()?;
    Ok(())
}

fn derive_b32(dest_b64: &str) -> String {
    use sha2::{Digest, Sha256};
    let raw = i2p_base64_decode(dest_b64);
    let hash = Sha256::digest(&raw);
    let b32 = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &hash);
    format!("{}.b32.i2p", b32.to_lowercase())
}

fn i2p_base64_decode(s: &str) -> Vec<u8> {
    use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
    let s = s.replace('-', "+").replace('~', "/");
    STANDARD_NO_PAD.decode(s).expect("decode base64")
}
