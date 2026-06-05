use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

pub struct FakeSam {
    pub addr: String,
    handle: thread::JoinHandle<()>,
}

impl FakeSam {
    pub fn spawn(handler: impl FnOnce(TcpStream) + Send + 'static) -> Self {
        Self::spawn_many(vec![Box::new(handler)])
    }

    pub fn spawn_many(mut handlers: Vec<Box<dyn FnOnce(TcpStream) + Send>>) -> Self {
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

    pub fn join(self) {
        self.handle.join().expect("fake SAM thread");
    }
}

pub fn expect_line(stream: &mut TcpStream, expected: &str) {
    let line = read_line(stream);
    assert_eq!(line, expected);
}

pub fn read_line(stream: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).expect("read line byte");
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8(buf).expect("utf-8 line").trim_end().to_string()
}
