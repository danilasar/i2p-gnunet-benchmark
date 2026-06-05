mod common;
use common::*;
use sam3::{SamClient, Keys};
use std::io::Write;

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
fn client_ping_parses_pong() {
    let server = FakeSam::spawn(|mut stream| {
        expect_line(&mut stream, "HELLO VERSION MIN=3.0 MAX=3.3");
        writeln!(stream, "HELLO REPLY RESULT=OK VERSION=3.3").unwrap();

        expect_line(&mut stream, "PINGhello");
        writeln!(stream, "PONGhello").unwrap();
    });

    let client = SamClient::connect(&server.addr);
    let pong = client.ping("hello").unwrap();
    assert_eq!(pong, "hello");
    server.join();
}

#[test]
fn keys_roundtrip_keyfile() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("keys.dat");
    let keys = Keys::new("public_key_data", "private_key_data");
    keys.write_keyfile(&path).unwrap();
    
    let loaded = Keys::read_keyfile(&path).unwrap();
    assert_eq!(keys, loaded);
}

#[test]
fn keys_read_private_only_keyfile() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("keys.dat");
    std::fs::write(&path, "PRIV=only_private\nPUB=only_public\n").unwrap();
    
    let loaded = Keys::read_keyfile(&path).unwrap();
    assert_eq!(loaded.destination().as_str(), "only_public");
    assert_eq!(loaded.private_key().as_str(), "only_private");
}
