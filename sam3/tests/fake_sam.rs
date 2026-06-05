use sam3::{SamClient, SamSession, SAM_TUNNEL_OPTIONS};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

#[test]
fn create_stream_sends_expected_sam_commands() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
        writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();

        let create = read_line(&mut stream);
        assert!(
            create.starts_with("SESSION CREATE STYLE=STREAM ID=test_session DESTINATION=privdest ")
        );
        assert!(create.contains("inbound.length=0"));
        assert!(create.contains("outbound.length=0"));
        assert!(create.contains("inbound.lengthVariance=0"));
        assert!(create.contains("outbound.lengthVariance=0"));
        assert!(create.contains("inbound.backupQuantity=0"));
        assert!(create.contains("outbound.backupQuantity=0"));
        assert!(create.contains("inbound.quantity=2"));
        assert!(create.contains("outbound.quantity=2"));
        assert!(create.ends_with("SIGNATURE_TYPE=7"));
        writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();

        let mut eof = [0u8; 1];
        assert_eq!(stream.read(&mut eof).unwrap(), 0);
    });

    let session = SamSession::create_stream(&server.addr, "test_session", SAM_TUNNEL_OPTIONS)
        .expect("create stream session");

    assert_eq!(session.destination, "pubdest");
    drop(session);
    server.join();
}

#[test]
fn sam_client_creates_stream_session_with_destination() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
        writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();

        let create = read_line(&mut stream);
        assert!(
            create.starts_with("SESSION CREATE STYLE=STREAM ID=api_session DESTINATION=privdest ")
        );
        writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();

        let mut eof = [0u8; 1];
        assert_eq!(stream.read(&mut eof).unwrap(), 0);
    });

    let client = SamClient::connect(&server.addr);
    let session = client
        .new_stream_session("api_session", SAM_TUNNEL_OPTIONS)
        .expect("create stream session");

    assert_eq!(session.id(), "api_session");
    assert_eq!(session.destination(), "pubdest");
    drop(session);
    server.join();
}

#[test]
fn connect_stream_sends_expected_sam_commands_and_returns_socket() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(
            &mut stream,
            "STREAM CONNECT ID=client DESTINATION=serverdest FROM_PORT=0 TO_PORT=0 SILENT=false",
        );
        writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();

        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).unwrap();
        assert_eq!(byte[0], b'x');
    });

    let mut conn =
        SamSession::connect_stream(&server.addr, "client", "serverdest").expect("connect stream");
    conn.write_all(b"x").unwrap();
    drop(conn);
    server.join();
}

#[test]
fn stream_session_dial_uses_session_id() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(
                &mut stream,
                "STREAM CONNECT ID=client DESTINATION=serverdest FROM_PORT=0 TO_PORT=0 SILENT=false",
            );
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);
    let client = SamClient::connect(&server.addr);
    let session = client
        .new_stream_session("client", SAM_TUNNEL_OPTIONS)
        .expect("create stream session");
    drop(session.dial("serverdest").expect("dial stream"));
    server.join();
}

#[test]
fn accept_stream_sends_expected_sam_commands_and_returns_socket() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "STREAM ACCEPT ID=server SILENT=false");
        writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        writeln!(stream, "REMOTE DESTINATION=clientdest").unwrap();
        stream.write_all(b"hello").unwrap();
    });

    let mut conn = SamSession::accept_stream(&server.addr, "server").expect("accept stream");
    let mut buf = [0u8; 5];
    conn.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"hello");
    drop(conn);
    server.join();
}

#[test]
fn stream_listener_accept_uses_session_id() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "STREAM ACCEPT ID=server SILENT=false");
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
            writeln!(stream, "REMOTE DESTINATION=clientdest").unwrap();
            stream.write_all(b"hello").unwrap();
        }),
    ]);
    let client = SamClient::connect(&server.addr);
    let session = client
        .new_stream_session("server", SAM_TUNNEL_OPTIONS)
        .expect("create stream session");
    let listener = session.listen();
    assert_eq!(listener.id(), "server");

    let mut conn = listener.accept().expect("accept stream");
    let mut buf = [0u8; 5];
    conn.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
    server.join();
}

#[test]
fn create_stream_returns_sam_errors() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
        writeln!(
            stream,
            "DEST REPLY RESULT=I2P_ERROR MESSAGE=\"key generation failed\""
        )
        .unwrap();
    });

    let err = match SamSession::create_stream(&server.addr, "test_session", SAM_TUNNEL_OPTIONS) {
        Ok(_) => panic!("create must fail"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("DEST GENERATE failed"));
    server.join();
}

struct FakeSam {
    addr: String,
    handle: thread::JoinHandle<()>,
}

impl FakeSam {
    fn spawn(handler: impl FnOnce(TcpStream) + Send + 'static) -> Self {
        Self::spawn_many(vec![Box::new(handler)])
    }

    fn spawn_many(mut handlers: Vec<Box<dyn FnOnce(TcpStream) + Send>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake SAM");
        let addr = listener.local_addr().expect("fake SAM addr").to_string();
        let handle = thread::spawn(move || {
            for handler in handlers.drain(..) {
                let (stream, _) = listener.accept().expect("accept fake SAM client");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("set read timeout");
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .expect("set write timeout");
                handler(stream);
            }
        });
        Self { addr, handle }
    }

    fn join(self) {
        self.handle.join().expect("fake SAM thread");
    }
}

fn expect_line(stream: &mut TcpStream, expected: &str) {
    let line = read_line(stream);
    assert_eq!(line, expected);
}

fn read_line(stream: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).expect("read line byte");
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8(buf).expect("utf-8 line")
}
