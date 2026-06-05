pub const SAM_VERSION_MIN: &str = "3.0";
pub const SAM_VERSION_MAX: &str = "3.3";
pub const DEFAULT_SIGNATURE_TYPE: &str = "7";
pub const SAM_UDP_PORT: u16 = 7655;

/// HELLO VERSION MIN=3.0 MAX=3.3\n
pub fn hello() -> String {
    format!("HELLO VERSION MIN={} MAX={}\n", SAM_VERSION_MIN, SAM_VERSION_MAX)
}

/// DEST GENERATE SIGNATURE_TYPE={sig_type}\n
pub fn dest_generate(sig_type: &str) -> String {
    format!("DEST GENERATE SIGNATURE_TYPE={}\n", sig_type)
}

/// SESSION CREATE STYLE=STREAM ID={id} DESTINATION={dest} SIGNATURE_TYPE=7 {opts}\n
pub fn session_create_stream(id: &str, dest: &str, opts: &str) -> String {
    let mut cmd = format!("SESSION CREATE STYLE=STREAM ID={} DESTINATION={} SIGNATURE_TYPE={}", id, dest, DEFAULT_SIGNATURE_TYPE);
    if !opts.is_empty() {
        cmd.push(' ');
        cmd.push_str(opts);
    }
    cmd.push('\n');
    cmd
}

/// SESSION CREATE STYLE=DATAGRAM ID={id} DESTINATION={dest} PORT={udp_port} HOST=127.0.0.1 SIGNATURE_TYPE=7 {opts}\n
pub fn session_create_datagram(id: &str, dest: &str, udp_port: u16, opts: &str) -> String {
    let mut cmd = format!(
        "SESSION CREATE STYLE=DATAGRAM ID={} DESTINATION={} PORT={} HOST=127.0.0.1 SIGNATURE_TYPE={}",
        id, dest, udp_port, DEFAULT_SIGNATURE_TYPE
    );
    if !opts.is_empty() {
        cmd.push(' ');
        cmd.push_str(opts);
    }
    cmd.push('\n');
    cmd
}

/// SESSION CREATE STYLE=RAW ID={id} DESTINATION={dest} PORT={udp_port} HOST=127.0.0.1 SIGNATURE_TYPE=7
/// [PROTOCOL={protocol}] [HEADER=true] {opts}\n
pub fn session_create_raw(
    id: &str,
    dest: &str,
    udp_port: u16,
    protocol: Option<u8>,
    header: bool,
    opts: &str,
) -> String {
    let mut cmd = format!(
        "SESSION CREATE STYLE=RAW ID={} DESTINATION={} PORT={} HOST=127.0.0.1 SIGNATURE_TYPE={}",
        id, dest, udp_port, DEFAULT_SIGNATURE_TYPE
    );
    if let Some(p) = protocol {
        cmd.push_str(&format!(" PROTOCOL={}", p));
    }
    if header {
        cmd.push_str(" HEADER=true");
    }
    if !opts.is_empty() {
        cmd.push(' ');
        cmd.push_str(opts);
    }
    cmd.push('\n');
    cmd
}

/// SESSION CREATE STYLE=PRIMARY ID={id} DESTINATION={dest} SIGNATURE_TYPE=7 {opts}\n
pub fn session_create_primary(id: &str, dest: &str, opts: &str) -> String {
    let mut cmd = format!("SESSION CREATE STYLE=PRIMARY ID={} DESTINATION={} SIGNATURE_TYPE={}", id, dest, DEFAULT_SIGNATURE_TYPE);
    if !opts.is_empty() {
        cmd.push(' ');
        cmd.push_str(opts);
    }
    cmd.push('\n');
    cmd
}

/// SESSION ADD STYLE=STREAM ID={id}\n
pub fn session_add_stream(id: &str) -> String {
    format!("SESSION ADD STYLE=STREAM ID={}\n", id)
}

/// SESSION ADD STYLE=DATAGRAM ID={id} PORT={udp_port}\n
pub fn session_add_datagram(id: &str, udp_port: u16) -> String {
    format!("SESSION ADD STYLE=DATAGRAM ID={} PORT={}\n", id, udp_port)
}

/// SESSION ADD STYLE=RAW ID={id} PORT={udp_port}\n
pub fn session_add_raw(id: &str, udp_port: u16) -> String {
    format!("SESSION ADD STYLE=RAW ID={} PORT={}\n", id, udp_port)
}

/// SESSION REMOVE ID={id}\n
pub fn session_remove(id: &str) -> String {
    format!("SESSION REMOVE ID={}\n", id)
}

/// STREAM CONNECT ID={id} DESTINATION={dest} SILENT={silent}
/// [FROM_PORT={from}] [TO_PORT={to}]\n
pub fn stream_connect(id: &str, dest: &str, silent: bool, from_port: u16, to_port: u16) -> String {
    let mut cmd = format!("STREAM CONNECT ID={} DESTINATION={} SILENT={}", id, dest, if silent { "true" } else { "false" });
    if from_port != 0 {
        cmd.push_str(&format!(" FROM_PORT={}", from_port));
    }
    if to_port != 0 {
        cmd.push_str(&format!(" TO_PORT={}", to_port));
    }
    cmd.push('\n');
    cmd
}

/// STREAM ACCEPT ID={id} SILENT={silent}\n
pub fn stream_accept(id: &str, silent: bool) -> String {
    format!("STREAM ACCEPT ID={} SILENT={}\n", id, if silent { "true" } else { "false" })
}

/// STREAM FORWARD ID={id} PORT={port} SILENT={silent} [HOST={host}]\n
pub fn stream_forward(id: &str, port: u16, host: Option<&str>, silent: bool) -> String {
    let mut cmd = format!("STREAM FORWARD ID={} PORT={} SILENT={}", id, port, if silent { "true" } else { "false" });
    if let Some(h) = host {
        cmd.push_str(&format!(" HOST={}", h));
    }
    cmd.push('\n');
    cmd
}

/// NAMING LOOKUP NAME={name}\n
pub fn naming_lookup(name: &str) -> String {
    format!("NAMING LOOKUP NAME={}\n", name)
}

/// PING{payload}\n
pub fn ping(payload: &str) -> String {
    format!("PING{}\n", payload)
}

/// Returns UDP datagram header (DATAGRAM): "3.1 {id} {dest}\n"
pub fn datagram_header(id: &str, dest: &str) -> String {
    format!("3.1 {} {}\n", id, dest)
}

/// Returns UDP packet header (RAW): "3.0 {id} {dest}\n"
pub fn raw_header(id: &str, dest: &str) -> String {
    format!("3.0 {} {}\n", id, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_format() {
        assert_eq!(hello(), "HELLO VERSION MIN=3.0 MAX=3.3\n");
    }

    #[test]
    fn dest_generate_default() {
        assert!(dest_generate("7").contains("SIGNATURE_TYPE=7"));
    }

    #[test]
    fn session_create_stream_no_opts() {
        assert_eq!(
            session_create_stream("id", "dest", ""),
            "SESSION CREATE STYLE=STREAM ID=id DESTINATION=dest SIGNATURE_TYPE=7\n"
        );
    }

    #[test]
    fn session_create_stream_with_opts() {
        assert_eq!(
            session_create_stream("id", "dest", "inbound.length=0"),
            "SESSION CREATE STYLE=STREAM ID=id DESTINATION=dest SIGNATURE_TYPE=7 inbound.length=0\n"
        );
    }

    #[test]
    fn session_create_datagram_contains_port_host() {
        let cmd = session_create_datagram("id", "dest", 7777, "");
        assert!(cmd.contains("PORT=7777"));
        assert!(cmd.contains("HOST=127.0.0.1"));
    }

    #[test]
    fn session_create_raw_no_protocol() {
        let cmd = session_create_raw("id", "dest", 7777, None, false, "");
        assert!(!cmd.contains("PROTOCOL="));
    }

    #[test]
    fn session_create_raw_with_protocol() {
        let cmd = session_create_raw("id", "dest", 7777, Some(18), false, "");
        assert!(cmd.contains("PROTOCOL=18"));
    }

    #[test]
    fn session_create_raw_with_header() {
        let cmd = session_create_raw("id", "dest", 7777, None, true, "");
        assert!(cmd.contains("HEADER=true"));
    }

    #[test]
    fn session_create_raw_no_header() {
        let cmd = session_create_raw("id", "dest", 7777, None, false, "");
        assert!(!cmd.contains("HEADER="));
    }

    #[test]
    fn session_add_stream_format() {
        assert_eq!(session_add_stream("x"), "SESSION ADD STYLE=STREAM ID=x\n");
    }

    #[test]
    fn session_add_datagram_format() {
        assert!(session_add_datagram("x", 7777).contains("PORT=7777"));
    }

    #[test]
    fn session_remove_format() {
        assert_eq!(session_remove("x"), "SESSION REMOVE ID=x\n");
    }

    #[test]
    fn stream_connect_no_ports() {
        let cmd = stream_connect("id", "dest", false, 0, 0);
        assert!(!cmd.contains("FROM_PORT="));
        assert!(!cmd.contains("TO_PORT="));
    }

    #[test]
    fn stream_connect_with_ports() {
        let cmd = stream_connect("id", "dest", false, 1234, 5678);
        assert!(cmd.contains("FROM_PORT=1234"));
        assert!(cmd.contains("TO_PORT=5678"));
    }

    #[test]
    fn stream_forward_no_host() {
        let cmd = stream_forward("id", 7777, None, false);
        assert!(!cmd.contains("HOST="));
    }

    #[test]
    fn stream_forward_with_host() {
        let cmd = stream_forward("id", 7777, Some("127.0.0.2"), false);
        assert!(cmd.contains("HOST=127.0.0.2"));
    }

    #[test]
    fn stream_forward_silent() {
        let cmd = stream_forward("id", 7777, None, true);
        assert!(cmd.contains("SILENT=true"));
    }

    #[test]
    fn ping_format() {
        assert_eq!(ping("hello"), "PINGhello\n");
    }

    #[test]
    fn datagram_header_format() {
        assert_eq!(datagram_header("myid", "dest123"), "3.1 myid dest123\n");
    }

    #[test]
    fn raw_header_format() {
        assert_eq!(raw_header("myid", "dest123"), "3.0 myid dest123\n");
    }

    #[test]
    fn session_create_primary_format() {
        let cmd = session_create_primary("id", "dest", "");
        assert!(cmd.contains("STYLE=PRIMARY"));
    }
}
