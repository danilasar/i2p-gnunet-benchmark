use std::process::Command;

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub index: usize,
    pub ns: String,
    pub ip: String,
    pub veth: String,
}

pub struct Topology {
    bridge: String,
    pub nodes: Vec<NodeInfo>,
}

impl Topology {
    pub fn new(n: usize, subnet: &str) -> Self {
        let prefix = subnet.replace('.', "_");
        let bridge = format!("br_{prefix}");
        let mut tp = Self {
            bridge,
            nodes: Vec::with_capacity(n),
        };

        tp.run("ip", &["link", "add", &tp.bridge, "type", "bridge"]);
        tp.run("ip", &["link", "set", &tp.bridge, "up"]);

        for i in 0..n {
            let ns = format!("ns_{prefix}{i}");
            let veth_host = format!("veth_{prefix}{i}_h");
            let veth_ns = format!("veth_{prefix}{i}_n");
            let ip_cidr = format!("{subnet}.{}/24", i + 1);

            tp.run("ip", &["netns", "add", &ns]);
            tp.run(
                "ip",
                &["netns", "exec", &ns, "ip", "link", "set", "lo", "up"],
            );
            tp.run(
                "ip",
                &[
                    "link", "add", &veth_host, "type", "veth", "peer", "name", &veth_ns,
                ],
            );
            tp.run("ip", &["link", "set", &veth_host, "master", &tp.bridge]);
            tp.run("ip", &["link", "set", &veth_host, "up"]);
            tp.run("ip", &["link", "set", &veth_ns, "netns", &ns]);
            tp.run(
                "ip",
                &[
                    "netns", "exec", &ns, "ip", "addr", "add", &ip_cidr, "dev", &veth_ns,
                ],
            );
            tp.run(
                "ip",
                &["netns", "exec", &ns, "ip", "link", "set", &veth_ns, "up"],
            );

            tp.nodes.push(NodeInfo {
                index: i,
                ns,
                ip: format!("{subnet}.{}", i + 1),
                veth: veth_ns,
            });
        }

        tp
    }

    fn run(&self, name: &str, args: &[&str]) {
        let out = Command::new(name)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("command {name} {args:?} failed to start: {e}"));
        if !out.status.success() {
            panic!(
                "command {name} {args:?} failed: {}\nstdout:\n{}\nstderr:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

impl Drop for Topology {
    fn drop(&mut self) {
        for node in self.nodes.iter().rev() {
            let _ = Command::new("ip").args(["netns", "del", &node.ns]).status();
        }
        let _ = Command::new("ip")
            .args(["link", "set", &self.bridge, "down"])
            .status();
        let _ = Command::new("ip")
            .args(["link", "del", &self.bridge, "type", "bridge"])
            .status();
    }
}
