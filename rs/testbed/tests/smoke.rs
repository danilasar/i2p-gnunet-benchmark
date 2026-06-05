use std::{error::Error, process::Command, thread, time::Duration};
use testbed::{
    node::{gnunet::GnunetPeer, i2pd::I2pdNode},
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
    for dep in ["ip", "gnunet-arm", "i2pd"] {
        which::which(dep).unwrap_or_else(|_| panic!("Dependency {dep} not found"));
    }
}

#[test]
fn test_gnunet_smoke() -> Result<(), Box<dyn Error>> {
    require_root();
    check_deps();
    let tp = Topology::new(2, "10.99.0");

    let mut peers = Vec::new();
    for i in 0..2 {
        let mut peer = GnunetPeer::new(
            i,
            tp.nodes[i].ns.clone(),
            tp.nodes[i].ip.clone(),
            2101 + i as u16,
        );
        peer.write_config()?;
        peer.start()?;
        peers.push(peer);
    }

    eprintln!("Waiting for GNUnet peers to initialize...");
    thread::sleep(Duration::from_secs(10));

    let hellos = peers
        .iter()
        .map(GnunetPeer::export_hello)
        .collect::<Result<Vec<_>, _>>()?;
    assert!(!hellos[0].is_empty());
    assert!(!hellos[1].is_empty());

    peers[0].import_hello(&hellos[1])?;
    peers[1].import_hello(&hellos[0])?;

    for peer in &peers {
        eprintln!("Waiting for peer {} to connect...", peer.index);
        peer.wait_core_connected(Duration::from_secs(60))?;
    }
    Ok(())
}

#[test]
fn test_i2pd_smoke() -> Result<(), Box<dyn Error>> {
    require_root();
    check_deps();
    let tp = Topology::new(4, "10.88.0");
    let tmp = tempfile::tempdir()?;

    let mut nodes = Vec::new();
    let mut node0 = I2pdNode::new(
        0,
        tp.nodes[0].ns.clone(),
        tp.nodes[0].ip.clone(),
        12000,
        true,
    );
    node0.sam_port = Some(17656);
    node0.write_config(None)?;
    node0.start()?;
    eprintln!("Waiting for floodfill node to generate RouterInfo...");
    node0.wait_router_info_floodfill(Duration::from_secs(90))?;
    nodes.push(node0);

    let zip_path = tmp.path().join("reseed.zip");
    nodes[0].create_reseed_zip(&zip_path)?;

    for i in 1..4 {
        let node = I2pdNode::new(
            i,
            tp.nodes[i].ns.clone(),
            tp.nodes[i].ip.clone(),
            12000 + i as u16,
            false,
        );
        node.write_config(Some(&zip_path))?;
        node.start()?;
        nodes.push(node);
    }

    for node in nodes.iter().skip(1) {
        eprintln!("Waiting for node {} to bootstrap...", node.index);
        node.wait_bootstrapped(Duration::from_secs(90))?;
    }

    check_sam(&tp.nodes[0].ns, 17656)?;
    Ok(())
}

fn check_sam(ns: &str, port: u16) -> Result<(), Box<dyn Error>> {
    let cmd = format!(
        "exec 3<>/dev/tcp/127.0.0.1/{port} && echo 'HELLO VERSION MIN=3.0 MAX=3.3' >&3 && head -n 1 <&3"
    );
    let out = Command::new("ip")
        .args(["netns", "exec", ns, "bash", "-c", &cmd])
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "SAM check failed: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("HELLO REPLY RESULT=OK"),
        "SAM stdout: {stdout}"
    );
    Ok(())
}
