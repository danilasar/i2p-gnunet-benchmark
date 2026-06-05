mod common;
use common::*;
use sam3::{SamClient, SamError, SessionOptions, Keys, Destination};
use std::io::Write;
use std::net::UdpSocket;
use std::time::Duration;

#[test]
fn datagram_session_create_sends_correct_commands() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        let create = read_line(&mut stream);
        assert!(create.starts_with("SESSION CREATE STYLE=DATAGRAM ID=dg "));
        assert!(create.contains("PORT="));
        writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=dgdest").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let keys = Keys::new("dgpub", "dgpriv");
    let session = client.new_datagram_session("dg", keys, &SessionOptions::default()).unwrap();
    assert_eq!(session.id(), "dg");
    server.join();
}

#[test]
fn datagram_send_to_writes_correct_packet_format() {
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
        .new_transient_datagram_session("dg_id", &SessionOptions::zero_hop())
        .unwrap();

    session.set_sam_udp_port(sam_udp_addr.port());

    let dest = Destination::new("target_dest");
    session.send_to(b"hello world", &dest).unwrap();

    let mut buf = [0u8; 1024];
    let (n, _) = sam_udp.recv_from(&mut buf).unwrap();
    let packet = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(packet, "3.1 dg_id target_dest\nhello world");
    server.join();
}

#[test]
fn datagram_recv_from_parses_sender_and_data() {
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
        .new_transient_datagram_session("dg_id", &SessionOptions::zero_hop())
        .unwrap();

    session.set_sam_udp_port(sam_udp_addr.port());

    let local_addr = session.local_addr().unwrap();
    sam_udp
        .send_to(b"sender_dest\nsecret data", local_addr)
        .unwrap();

    let mut buf = [0u8; 1024];
    let (n, sender) = session.recv_from(&mut buf).unwrap();
    assert_eq!(sender.as_str(), "sender_dest");
    assert_eq!(&buf[..n], b"secret data");
    server.join();
}

#[test]
fn datagram_recv_from_ignores_packets_not_from_sam_ip() {
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
        .new_transient_datagram_session("dg_id", &SessionOptions::zero_hop())
        .unwrap();

    session.set_sam_udp_port(sam_udp_addr.port());

    session.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let local_addr = session.local_addr().unwrap();

    let wrong_udp = UdpSocket::bind("127.0.0.2:0").unwrap();
    let _ = wrong_udp.send_to(b"hacker\nfake data", local_addr);

    sam_udp
        .send_to(b"real_sender\ncorrect data", local_addr)
        .unwrap();

    let mut buf = [0u8; 1024];
    let (n, sender) = session.recv_from(&mut buf).unwrap();
    assert_eq!(sender.as_str(), "real_sender");
    assert_eq!(&buf[..n], b"correct data");
    server.join();
}

#[test]
fn datagram_send_exceeds_limit() {
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
    let session = client.new_transient_datagram_session("dg", &SessionOptions::default()).unwrap();
    
    let large_data = vec![0u8; 31745];
    let res = session.send_to(&large_data, &"dest".into());
    assert!(matches!(res, Err(SamError::DatagramTooLarge { max: 31744, got: 31745 })));
    server.join();
}
