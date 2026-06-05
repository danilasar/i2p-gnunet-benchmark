mod common;
use common::*;
use sam3::{SamClient, SessionOptions, StreamSession};
use std::io::Write;

#[test]
fn primary_session_create_and_sub_sessions() {
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

            let create = read_line(&mut stream);
            assert!(create.contains("STYLE=PRIMARY"));
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=primdest").unwrap();

            let add = read_line(&mut stream);
            assert!(add.contains("SESSION ADD STYLE=STREAM ID=sub-stream"));
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=subdest").unwrap();

            let remove = read_line(&mut stream);
            assert!(remove.contains("SESSION REMOVE ID=sub-stream"));
            writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let mut session = client.new_transient_primary_session("prim", &SessionOptions::default()).unwrap();
    
    let sub: StreamSession = session.new_stream_sub_session("sub-stream").unwrap();
    assert_eq!(sub.id(), "sub-stream");
    
    session.remove_sub_session("sub-stream").unwrap();
    server.join();
}

#[test]
fn primary_ping_pong() {
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
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=dest").unwrap();

            expect_line(&mut stream, "PINGtest");
            writeln!(stream, "PONGtest").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let session = client.new_transient_primary_session("prim", &SessionOptions::default()).unwrap();
    let pong = session.ping("test").unwrap();
    assert_eq!(pong, "test");
    server.join();
}

#[test]
fn primary_session_remove_sends_correct_command() {
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
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=dest").unwrap();

            expect_line(&mut stream, "SESSION REMOVE ID=sub-id");
            writeln!(stream, "SESSION STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let mut session = client.new_transient_primary_session("prim", &SessionOptions::default()).unwrap();
    session.remove_sub_session("sub-id").unwrap();
    server.join();
}

#[test]
fn primary_stream_sub_session_can_connect() {
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
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=primdest").unwrap();
            let _ = read_line(&mut stream);
            writeln!(stream, "SESSION STATUS RESULT=OK DESTINATION=subdest").unwrap();
        }),
        Box::new(|mut stream| {
            expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
            writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
            expect_line(
                &mut stream,
                "STREAM CONNECT ID=sub_stream DESTINATION=target_dest SILENT=false",
            );
            writeln!(stream, "STREAM STATUS RESULT=OK").unwrap();
        }),
    ]);

    let client = SamClient::connect(&server.addr);
    let mut primary = client
        .new_transient_primary_session("prim_test", &SessionOptions::zero_hop())
        .unwrap();
    let sub = primary.new_stream_sub_session("sub_stream").unwrap();
    let _conn = sub.dial("target_dest").unwrap();
    server.join();
}
