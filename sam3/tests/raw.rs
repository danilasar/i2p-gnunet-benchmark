mod common;
use common::*;
use sam3::{SamClient, SamError, SessionOptions, Keys, Destination};
use std::io::Write;
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn raw_session_create_sends_correct_commands() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        let create = read_line(&mut stream);
        assert!(create.starts_with("SESSION CREATE STYLE=RAW ID=raw "));
        assert!(create.contains("PROTOCOL=18"));
        assert!(create.contains("HEADER=true"));
        writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=rawdest").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let keys = Keys::new("rawpub", "rawpriv");
    let mut raw_opts = sam3::RawSessionOptions::default();
    raw_opts.protocol = Some(18);
    raw_opts.header = true;
    let session = client.new_raw_session("raw", keys, &SessionOptions::default(), &raw_opts).unwrap();
    assert_eq!(session.id(), "raw");
    server.join();
}

#[test]
fn raw_send_to_writes_correct_packet_format() {
    let sam_udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sam_udp_addr = sam_udp.local_addr().unwrap();
    sam_udp.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let mut session = client
        .new_transient_raw_session("raw_id", &SessionOptions::zero_hop())
        .unwrap();

    session.set_sam_udp_port(sam_udp_addr.port());

    let dest = Destination::new("target_dest");
    session.send_to(b"hello world", &dest).unwrap();

    let mut buf = [0u8; 1024];
    let (n, _) = sam_udp.recv_from(&mut buf).unwrap();
    let packet = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(packet, "3.0 raw_id target_dest\nhello world");
    server.join();
}

#[test]
fn raw_read_returns_payload_without_header() {
    let sam_udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sam_udp_addr = sam_udp.local_addr().unwrap();

    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let mut session = client
        .new_transient_raw_session("raw_id", &SessionOptions::zero_hop())
        .unwrap();

    session.set_sam_udp_port(sam_udp_addr.port());

    let local_addr = session.local_addr().unwrap();
    sam_udp.send_to(b"raw data", local_addr).unwrap();

    let mut buf = [0u8; 1024];
    let n = session.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"raw data");
    server.join();
}

#[test]
fn raw_read_ignores_packets_not_from_sam_ip() {
    let sam_udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sam_udp_addr = sam_udp.local_addr().unwrap();

    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let mut session = client
        .new_transient_raw_session("raw_id", &SessionOptions::zero_hop())
        .unwrap();

    session.set_sam_udp_port(sam_udp_addr.port());

    session.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let local_addr = session.local_addr().unwrap();

    let wrong_udp = UdpSocket::bind("127.0.0.2:0").unwrap();
    let _ = wrong_udp.send_to(b"fake data", local_addr);

    sam_udp.send_to(b"correct data", local_addr).unwrap();

    let mut buf = [0u8; 1024];
    let n = session.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"correct data");
    server.join();
}

#[test]
fn raw_send_exceeds_limit() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client.new_transient_raw_session("raw", &SessionOptions::default()).unwrap();
    
    let large_data = vec![0u8; 32769];
    let res = session.send_to(&large_data, &"dest".into());
    assert!(matches!(res, Err(SamError::DatagramTooLarge { max: 32768, got: 32769 })));
    server.join();
}
