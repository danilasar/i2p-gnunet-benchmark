use sha2::{Digest, Sha256};
use std::io::{self, Read};

pub struct PayloadReader {
    size: i64,
    seed: i64,
    pos: i64,
}

impl PayloadReader {
    pub fn new(size: i64, seed: i64) -> Self {
        Self { size, seed, pos: 0 }
    }
}

impl Read for PayloadReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.size {
            return Ok(0);
        }
        let mut n = 0;
        while n < buf.len() && self.pos < self.size {
            buf[n] = ((self.pos * self.seed + self.pos) % 256) as u8;
            self.pos += 1;
            n += 1;
        }
        Ok(n)
    }
}

pub fn expected_sha256(size: i64, seed: i64) -> [u8; 32] {
    let mut h = Sha256::new();
    let mut r = PayloadReader::new(size, seed);
    std::io::copy(&mut r, &mut h).expect("hash payload");
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::{expected_sha256, PayloadReader};
    use sha2::{Digest, Sha256};
    use std::io::Read;

    #[test]
    fn payload_reader_is_deterministic() {
        let mut reader = PayloadReader::new(8, 42);
        let mut buf = Vec::new();

        reader.read_to_end(&mut buf).expect("read payload");

        assert_eq!(buf, vec![0, 43, 86, 129, 172, 215, 2, 45]);
    }

    #[test]
    fn expected_sha256_matches_generated_payload() {
        let mut reader = PayloadReader::new(1024, 42);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).expect("read payload");

        let actual: [u8; 32] = Sha256::digest(&bytes).into();

        assert_eq!(expected_sha256(1024, 42), actual);
    }
}
