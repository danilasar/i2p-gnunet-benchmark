use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

const I2PD_TEMPLATE: &str = include_str!("../../templates/i2pd.conf.tmpl");

pub struct I2pdNode {
    pub index: usize,
    pub data_dir: TempDir,
    pub conf_path: PathBuf,
    pub ns: String,
    pub ip: String,
    pub port: u16,
    pub floodfill: bool,
    pub sam_port: Option<u16>,
}

impl I2pdNode {
    pub fn new(
        index: usize,
        ns: impl Into<String>,
        ip: impl Into<String>,
        port: u16,
        floodfill: bool,
    ) -> Self {
        let data_dir = tempfile::tempdir().expect("create i2pd tempdir");
        fs::create_dir_all(data_dir.path().join("netDb")).expect("create netDb");
        let conf_path = data_dir.path().join("i2pd.conf");
        Self {
            index,
            data_dir,
            conf_path,
            ns: ns.into(),
            ip: ip.into(),
            port,
            floodfill,
            sam_port: None,
        }
    }

    pub fn write_config(&self, zip_file: Option<&Path>) -> Result<(), Box<dyn Error>> {
        let sam_enabled = self.sam_port.is_some();
        let rendered = minijinja::render!(
            I2PD_TEMPLATE,
            host => self.ip,
            zip_file => zip_file.map(|p| p.display().to_string()).unwrap_or_default(),
            port => self.port,
            sam_enabled => sam_enabled,
            sam_port => self.sam_port.unwrap_or(0),
        );
        fs::write(&self.conf_path, rendered)?;
        Ok(())
    }

    pub fn start(&self) -> Result<(), Box<dyn Error>> {
        let mut args = vec![
            "netns".to_string(),
            "exec".to_string(),
            self.ns.clone(),
            "i2pd".to_string(),
            format!("--datadir={}", self.data_dir.path().display()),
            format!("--conf={}", self.conf_path.display()),
            format!("--address4={}", self.ip),
            "--loglevel=info".to_string(),
            "--log=file".to_string(),
            format!(
                "--logfile={}",
                self.data_dir.path().join("i2pd.log").display()
            ),
            "--daemon".to_string(),
        ];
        if self.floodfill {
            args.push("--floodfill".to_string());
        }
        let out = Command::new("ip").args(&args).output()?;
        if !out.status.success() {
            return Err(format!(
                "failed to start i2pd: {}\nstdout:\n{}\nstderr:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
        Ok(())
    }

    pub fn stop(&self) {
        let needle = format!("datadir={}", self.data_dir.path().display());
        let _ = Command::new("pkill").args(["-9", "-f", &needle]).status();
        for _ in 0..50 {
            thread::sleep(Duration::from_millis(200));
            let out = Command::new("pgrep").args(["-f", &needle]).output();
            if out
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().is_empty())
                .unwrap_or(true)
            {
                break;
            }
        }
        let _ = fs::remove_file(self.data_dir.path().join("i2pd.pid"));
    }

    pub fn router_info_path(&self) -> PathBuf {
        self.data_dir.path().join("router.info")
    }

    pub fn wait_router_info_floodfill(&self, timeout: Duration) -> Result<(), Box<dyn Error>> {
        if !self.floodfill {
            return Ok(());
        }
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(data) = fs::read(self.router_info_path()) {
                if data.windows(2).any(|w| w == b"Xf") {
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err("timeout waiting for floodfill router.info".into())
    }

    pub fn wait_bootstrapped(&self, timeout: Duration) -> Result<(), Box<dyn Error>> {
        let log_file = self.data_dir.path().join("i2pd.log");
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(data) = fs::read_to_string(&log_file) {
                if data.contains("NetDbReq: Exploring new")
                    || data.contains("Tunnel: all tunnels built")
                {
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(format!("timeout waiting for node {} bootstrap", self.index).into())
    }

    pub fn create_reseed_zip(&self, dest: &Path) -> Result<(), Box<dyn Error>> {
        create_multi_reseed_zip(dest, &[self])
    }
}

impl Drop for I2pdNode {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn create_multi_reseed_zip(dest: &Path, nodes: &[&I2pdNode]) -> Result<(), Box<dyn Error>> {
    let file = fs::File::create(dest)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for node in nodes {
        let (b64, data) = router_info_i2p_hash(&node.router_info_path())?;
        archive.start_file(format!("routerInfo-{b64}.dat"), options)?;
        archive.write_all(&data)?;
    }
    archive.finish()?;
    Ok(())
}

fn router_info_i2p_hash(path: &Path) -> Result<(String, Vec<u8>), Box<dyn Error>> {
    let mut data = Vec::new();
    fs::File::open(path)?.read_to_end(&mut data)?;
    if data.len() < 387 {
        return Err("router.info too short".into());
    }
    let cert_len = ((data[385] as usize) << 8) | data[386] as usize;
    let identity_len = 387 + cert_len;
    if data.len() < identity_len {
        return Err("router.info too short for identity".into());
    }
    let hash = Sha256::digest(&data[..identity_len]);
    let b64 = STANDARD_NO_PAD
        .encode(hash)
        .replace('+', "-")
        .replace('/', "~");
    Ok((b64, data))
}
