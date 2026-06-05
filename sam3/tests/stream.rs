mod common;
use common::*;
use sam3::{SamClient, SamError, SessionOptions};
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn stream_forward_sends_port_and_silent() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream); // SESSION CREATE
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "STREAM FORWARD ID=client PORT=8080 SILENT=true");
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client.new_transient_stream_session("client", &SessionOptions::zero_hop()).unwrap();
    session.forward(8080, true).unwrap();
    server.join();
}

#[test]
fn connect_stream_returns_cant_reach_peer() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream); // SESSION CREATE
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            let _ = read_line(&mut stream); // STREAM CONNECT ...
            writeln!(stream, "STREAM STATUS RESULT=CANT_REACH_PEER").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client.new_transient_stream_session("client", &SessionOptions::zero_hop()).unwrap();
    let res = session.dial("dest");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), SamError::CantReachPeer);
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
            let _ = read_line(&mut stream); // SESSION CREATE
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(
                &mut stream,
                "STREAM CONNECT ID=client DESTINATION=serverdest SILENT=false",
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
fn stream_session_dial_with_ports() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream); // SESSION CREATE
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(
                &mut stream,
                "STREAM CONNECT ID=client DESTINATION=dest SILENT=true FROM_PORT=1234 TO_PORT=5678",
            );
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client.new_transient_stream_session("client", &SessionOptions::zero_hop()).unwrap();
    let mut opts = sam3::StreamConnectOptions::default();
    opts.from_port = 1234;
    opts.to_port = 5678;
    opts.silent = true;
    session.dial_with_options("dest", &opts).unwrap();
    server.join();
}

#[test]
fn stream_forward_with_host() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream); // SESSION CREATE
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "STREAM FORWARD ID=client PORT=7777 SILENT=true HOST=127.0.0.2");
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client.new_transient_stream_session("client", &SessionOptions::zero_hop()).unwrap();
    session.forward_to(7777, Some("127.0.0.2"), true).unwrap();
    server.join();
}

#[test]
fn stream_forward_guard_drop_closes_connection() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(
                &mut stream,
                "STREAM FORWARD ID=test_session PORT=8080 SILENT=false",
            );
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();

            let mut buf = [0u8; 1];
            let res = stream.read(&mut buf);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), 0); // EOF
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("test_session", &SessionOptions::zero_hop())
        .unwrap();
    {
        let _guard = session.forward(8080, false).unwrap();
    }
    server.join();
}

#[test]
fn stream_listener_incoming_yields_connections() {
    let server = FakeSam::spawn_many(vec![
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "DEST GENERATE SIGNATURE_TYPE=7");
            writeln!(stream, "DEST REPLY PUB=pubdest PRIV=privdest").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=pubdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "STREAM ACCEPT ID=test_session SILENT=false");
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
            writeln!(stream, "remote_dest_1").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(&mut stream, "STREAM ACCEPT ID=test_session SILENT=false");
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
            writeln!(stream, "remote_dest_2").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client
        .new_transient_stream_session("test_session", &SessionOptions::zero_hop())
        .unwrap();

    let listener = session.listen();
    let mut incoming = listener.incoming();

    let conn1 = incoming.next().unwrap().unwrap();
    assert_eq!(conn1.remote_destination().as_str(), "remote_dest_1");

    let conn2 = incoming.next().unwrap().unwrap();
    assert_eq!(conn2.remote_destination().as_str(), "remote_dest_2");

    server.join();
}
