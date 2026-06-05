use sam3::{Destination, Keys, SamClient, SamError, SamSession, SessionOptions};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

#[test]
fn connect_stream_returns_cant_reach_peer() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        let _ = read_line(&mut stream);
        writeln!(stream, "STREAM STATUS RESULT=CANT_REACH_PEER").unwrap();
    });

    let err = SamSession::connect_stream(&server.addr, "client", "dest").unwrap_err();
    assert_eq!(err, SamError::CantReachPeer);
    server.join();
}

#[test]
fn session_create_returns_duplicated_id() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        let _ = read_line(&mut stream);
        writeln!(stream, "SESSION STATUS RESULT=DUPLICATED_ID").unwrap();
    });

    let err = SamSession::create_stream(&server.addr, "dup", &SessionOptions::zero_hop()).unwrap_err();
    assert_eq!(err, SamError::DuplicatedId);
    server.join();
}

#[test]
fn samconn_set_timeout_sets_both_directions() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        expect_line(&mut stream, "STREAM ACCEPT ID=server SILENT=false");
        writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        writeln!(stream, "clientdest").unwrap();
    });

    let local_dest = Destination::new("localdest");
    let conn = SamSession::accept_stream(&server.addr, "server", &local_dest).unwrap();
    conn.set_timeout(Some(Duration::from_secs(5))).unwrap();
    conn.set_read_timeout(None).unwrap();
    server.join();
}

#[test]
fn dial_timeout_succeeds_within_deadline() {
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
        .new_transient_stream_session("client", &SessionOptions::zero_hop())
        .expect("create stream session");
    let conn = session.dial_timeout("serverdest", Duration::from_secs(1)).unwrap();
    assert_eq!(conn.remote_destination().as_str(), "serverdest");
    server.join();
}

#[test]
fn dial_timeout_returns_error_on_slow_response() {
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
            let _ = read_line(&mut stream);
            thread::sleep(Duration::from_secs(1));
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("client", &SessionOptions::zero_hop())
        .expect("create stream session");
    let err = session.dial_timeout("serverdest", Duration::from_millis(100)).unwrap_err();
    match err {
        SamError::Io(_) => (),
        _ => panic!("expected Io error for timeout, got {:?}", err),
    }
    server.join();
}

#[test]
fn accept_timeout_succeeds_when_client_connects() {
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
            writeln!(stream, "clientdest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("server", &SessionOptions::zero_hop())
        .expect("create stream session");
    let listener = session.listen();
    let conn = listener.accept_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(conn.remote_destination().as_str(), "clientdest");
    server.join();
}

#[test]
fn accept_timeout_returns_error_when_no_client() {
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
            thread::sleep(Duration::from_secs(1));
            writeln!(stream, "clientdest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("server", &SessionOptions::zero_hop())
        .expect("create stream session");
    let listener = session.listen();
    let err = listener.accept_timeout(Duration::from_millis(100)).unwrap_err();
    match err {
        SamError::Io(_) => (),
        _ => panic!("expected Io error for timeout, got {:?}", err),
    }
    server.join();
}

#[test]
fn lookup_resolves_name_to_destination() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "NAMING LOOKUP NAME=example.i2p");
        writeln!(stream, "NAMING REPLY RESULT=OK NAME=example.i2p VALUE=base64dest").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let dest = client.lookup("example.i2p").unwrap();
    assert_eq!(dest.as_str(), "base64dest");
    server.join();
}

#[test]
fn lookup_returns_key_not_found() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "NAMING LOOKUP NAME=unknown.i2p");
        writeln!(stream, "NAMING REPLY RESULT=KEY_NOT_FOUND NAME=unknown.i2p").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let err = client.lookup("unknown.i2p").unwrap_err();
    assert_eq!(err, SamError::KeyNotFound);
    server.join();
}

#[test]
fn lookup_returns_invalid_key() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "NAMING LOOKUP NAME=bad");
        writeln!(stream, "NAMING REPLY RESULT=INVALID_KEY NAME=bad").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let err = client.lookup("bad").unwrap_err();
    assert_eq!(err, SamError::InvalidKey);
    server.join();
}

#[test]
fn lookup_returns_i2p_error_with_message() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "NAMING LOOKUP NAME=x");
        writeln!(stream, "NAMING REPLY RESULT=I2P_ERROR NAME=x MESSAGE=\"router error\"").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let err = client.lookup("x").unwrap_err();
    assert_eq!(err, SamError::I2PError("router error".into()));
    server.join();
}

#[test]
fn stream_session_lookup_uses_sam_addr() {
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
            expect_line(&mut stream, "NAMING LOOKUP NAME=example.i2p");
            writeln!(stream, "NAMING REPLY RESULT=OK NAME=example.i2p VALUE=base64dest").unwrap();
        }),
    ]);
    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("client", &SessionOptions::zero_hop())
        .expect("create stream session");
    let dest = session.lookup("example.i2p").unwrap();
    assert_eq!(dest.as_str(), "base64dest");
    server.join();
}

#[test]
fn accept_exposes_remote_destination() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "STREAM ACCEPT ID=server SILENT=false");
        writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        writeln!(stream, "clientdest").unwrap();
    });

    let local_dest = Destination::new("localdest");
    let conn = SamSession::accept_stream(&server.addr, "server", &local_dest).unwrap();
    assert_eq!(conn.remote_destination().as_str(), "clientdest");
    assert_eq!(conn.local_destination().as_str(), "localdest");
    server.join();
}

#[test]
fn dial_exposes_local_and_remote_destination() {
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
                "STREAM CONNECT ID=client DESTINATION=remotedest FROM_PORT=0 TO_PORT=0 SILENT=false",
            );
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);
    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("client", &SessionOptions::zero_hop())
        .expect("create stream session");
    let conn = session.dial("remotedest").unwrap();
    assert_eq!(conn.remote_destination().as_str(), "remotedest");
    assert_eq!(conn.local_destination().as_str(), "pubdest");
    server.join();
}

#[test]
fn samconn_read_write_delegates_to_socket() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "STREAM ACCEPT ID=server SILENT=false");
        writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        writeln!(stream, "clientdest").unwrap();
        stream.write_all(b"hello").unwrap();
    });

    let local_dest = Destination::new("localdest");
    let mut conn = SamSession::accept_stream(&server.addr, "server", &local_dest).unwrap();
    let mut buf = [0u8; 5];
    conn.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
    server.join();
}

#[test]
fn samconn_set_timeout_does_not_error() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "STREAM ACCEPT ID=server SILENT=false");
        writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        writeln!(stream, "clientdest").unwrap();
    });

    let local_dest = Destination::new("localdest");
    let conn = SamSession::accept_stream(&server.addr, "server", &local_dest).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    conn.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    server.join();
}

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

    let session = SamSession::create_stream(&server.addr, "test_session", &SessionOptions::zero_hop())
        .expect("create stream session");

    assert_eq!(session.destination().as_str(), "pubdest");
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
        .new_transient_stream_session("api_session", &SessionOptions::zero_hop())
        .expect("create stream session");

    assert_eq!(session.id(), "api_session");
    assert_eq!(session.destination().as_str(), "pubdest");
    drop(session);
    server.join();
}

#[test]
fn sam_client_generates_keys() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
        writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let keys = client.new_keys().expect("generate keys");

    assert_eq!(keys.destination().as_str(), "pubdest");
    assert_eq!(keys.private_key().as_str(), "privdest");
    server.join();
}

#[test]
fn sam_client_generates_keys_with_signature_type() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=11");
        writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let keys = client
        .new_keys_with_signature_type("11")
        .expect("generate keys");

    assert_eq!(keys.destination().as_str(), "pubdest");
    assert_eq!(keys.private_key().as_str(), "privdest");
    server.join();
}

#[test]
fn sam_client_creates_stream_session_with_existing_keys() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        let create = read_line(&mut stream);
        assert!(create
            .starts_with("SESSION CREATE STYLE=STREAM ID=keyed_session DESTINATION=existingpriv "));
        assert!(create.ends_with("SIGNATURE_TYPE=7"));
        writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let keys = Keys::new("existingpub", "existingpriv");
    let session = client
        .new_stream_session("keyed_session", &keys, &SessionOptions::zero_hop())
        .expect("create keyed stream session");

    assert_eq!(session.destination().as_str(), "existingpub");
    assert_eq!(session.keys(), &keys);
    drop(session);
    server.join();
}

#[test]
fn sam_client_ensure_keyfile_generates_missing_keys() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
        writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
    });
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("keys.dat");

    let client = SamClient::connect(&server.addr);
    let keys = client.ensure_keyfile(&path).expect("ensure keyfile");
    let loaded = Keys::read_keyfile(&path).expect("read keyfile");

    assert_eq!(keys, loaded);
    server.join();
}

#[test]
fn sam_client_ensure_keyfile_reuses_existing_keys() {
    let server = FakeSam::spawn_many(Vec::new());
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("keys.dat");
    let keys = Keys::new("existingpub", "existingpriv");
    keys.write_keyfile(&path).expect("write keyfile");

    let client = SamClient::connect(&server.addr);
    let loaded = client.ensure_keyfile(&path).expect("ensure keyfile");

    assert_eq!(loaded, keys);
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
        .new_transient_stream_session("client", &SessionOptions::zero_hop())
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
        writeln!(stream, "clientdest").unwrap();
        stream.write_all(b"hello").unwrap();
    });

    let local_dest = Destination::new("localdest");
    let mut conn = SamSession::accept_stream(&server.addr, "server", &local_dest).expect("accept stream");
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
            writeln!(stream, "clientdest").unwrap();
            stream.write_all(b"hello").unwrap();
        }),
    ]);
    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("server", &SessionOptions::zero_hop())
        .expect("create stream session");
    let listener = session.listen();
    assert_eq!(listener.id(), "server");

    let mut conn = listener.accept().expect("accept stream");
    assert_eq!(conn.local_destination().as_str(), "pubdest");
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

    let err = match SamSession::create_stream(&server.addr, "test_session", &SessionOptions::zero_hop()) {
        Ok(_) => panic!("create must fail"),
        Err(err) => err,
    };

    assert_eq!(err, SamError::I2PError("key generation failed".into()));
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
        let local_addr = listener.local_addr().expect("fake SAM addr");
        let addr = format!("127.0.0.1:{}", local_addr.port());
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

#[test]
fn session_create_uses_options_from_session_options() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
        writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();

        let create = read_line(&mut stream);
        assert!(create.contains("inbound.length=1"));
        assert!(create.contains("outbound.length=1"));
        assert!(!create.contains("inbound.quantity="));
        writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let _ = client
        .new_transient_stream_session(
            "test",
            &SessionOptions::default().inbound_length(1).outbound_length(1),
        )
        .unwrap();
    server.join();
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
