use std::{
    error::Error,
    fs,
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

const GNUNET_TEMPLATE: &str = include_str!("../../templates/gnunet.conf.tmpl");

pub struct GnunetPeer {
    pub index: usize,
    pub data_dir: TempDir,
    pub conf_path: PathBuf,
    pub ns: String,
    pub ip: String,
    pub port: u16,
    child: Option<Child>,
}

impl GnunetPeer {
    pub fn new(index: usize, ns: impl Into<String>, ip: impl Into<String>, port: u16) -> Self {
        let data_dir = tempfile::tempdir().expect("create gnunet tempdir");
        for rel in [".cache/gnunet", "data/hosts", "run"] {
            fs::create_dir_all(data_dir.path().join(rel)).expect("create gnunet dir");
        }
        let conf_path = data_dir.path().join("peer.conf");
        Self {
            index,
            data_dir,
            conf_path,
            ns: ns.into(),
            ip: ip.into(),
            port,
            child: None,
        }
    }

    pub fn write_config(&self) -> Result<(), Box<dyn Error>> {
        let rendered = minijinja::render!(
            GNUNET_TEMPLATE,
            data_dir => self.data_dir.path().display().to_string(),
            ip => self.ip,
            port => self.port,
        );
        fs::write(&self.conf_path, rendered)?;
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let child = Command::new("ip")
            .args([
                "netns",
                "exec",
                &self.ns,
                "gnunet-arm",
                "-c",
                self.conf_path.to_str().unwrap(),
                "-s",
                "-L",
                "DEBUG",
            ])
            .spawn()?;
        self.child = Some(child);
        Ok(())
    }

    pub fn export_hello(&self) -> Result<String, Box<dyn Error>> {
        let out = Command::new("ip")
            .args([
                "netns",
                "exec",
                &self.ns,
                "gnunet-hello",
                "-c",
                self.conf_path.to_str().unwrap(),
                "-e",
            ])
            .output()?;
        if !out.status.success() {
            return Err(format!(
                "failed to export hello: {}",
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn import_hello(&self, hello: &str) -> Result<(), Box<dyn Error>> {
        let mut child = Command::new("ip")
            .args([
                "netns",
                "exec",
                &self.ns,
                "gnunet-hello",
                "-c",
                self.conf_path.to_str().unwrap(),
                "--import",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.as_mut().unwrap().write_all(hello.as_bytes())?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            return Err(format!(
                "failed to import hello: {}",
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
        Ok(())
    }

    pub fn wait_core_connected(&self, timeout: Duration) -> Result<(), Box<dyn Error>> {
        let log_file = self.data_dir.path().join(".cache/gnunet/gnunet.log");
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(data) = fs::read_to_string(&log_file) {
                if data.contains("notification about connection")
                    || data.contains("connection established")
                    || data.contains("connected to")
                {
                    return Ok(());
                }
            }

            let out = Command::new("ip")
                .args([
                    "netns",
                    "exec",
                    &self.ns,
                    "gnunet-statistics",
                    "-c",
                    self.conf_path.to_str().unwrap(),
                    "-q",
                    "-s",
                    "core",
                ])
                .output();
            if let Ok(out) = out {
                let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
                if s.contains("neighbour") || s.contains("connection") {
                    return Ok(());
                }
            }

            thread::sleep(Duration::from_secs(2));
        }
        Err(format!(
            "timeout waiting for GNUnet peer {} core connection",
            self.index
        )
        .into())
    }
}

impl Drop for GnunetPeer {
    fn drop(&mut self) {
        let _ = Command::new("ip")
            .args([
                "netns",
                "exec",
                &self.ns,
                "gnunet-arm",
                "-c",
                self.conf_path.to_str().unwrap(),
                "-e",
            ])
            .status();
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}
