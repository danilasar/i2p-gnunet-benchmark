use sha2::{Digest, Sha256};
use std::{
    error::Error,
    io::{self, Read, Write},
    time::Instant,
};

pub fn send_payload(
    conn: &mut impl Write,
    reader: &mut impl Read,
    size: i64,
) -> Result<(i64, [u8; 32]), Box<dyn Error>> {
    conn.write_all(&(size as u64).to_be_bytes())?;
    let mut h = Sha256::new();
    let mut limited = reader.take(size as u64);
    let written = io::copy(
        &mut limited,
        &mut TeeWriter {
            inner: conn,
            hasher: &mut h,
        },
    )? as i64;
    if written != size {
        return Err(format!("short payload reader: wrote {written}, expected {size}").into());
    }
    let sum: [u8; 32] = h.finalize().into();
    conn.write_all(&sum)?;
    conn.flush()?;
    Ok((written, sum))
}

pub fn receive_payload(conn: &mut impl Read) -> Result<(i64, bool, Instant), Box<dyn Error>> {
    let mut size_buf = [0u8; 8];
    conn.read_exact(&mut size_buf)
        .map_err(|e| format!("failed to read size: {e}"))?;
    let size = u64::from_be_bytes(size_buf);

    let mut h = Sha256::new();
    let mut first = [0u8; 1];
    conn.read_exact(&mut first)
        .map_err(|e| format!("failed to read first byte: {e}"))?;
    let first_byte_at = Instant::now();
    h.update(first);

    let mut received = 1i64;
    if size > 1 {
        let mut limited = conn.take(size - 1);
        let nr = io::copy(&mut limited, &mut h)? as i64;
        received += nr;
        if nr != (size - 1) as i64 {
            return Err(format!("failed to read payload: short read {nr}/{}", size - 1).into());
        }
    }

    let mut sender_sum = [0u8; 32];
    conn.read_exact(&mut sender_sum)
        .map_err(|e| format!("failed to read sha256: {e}"))?;
    let my_sum: [u8; 32] = h.finalize().into();
    Ok((received, sender_sum == my_sum, first_byte_at))
}

struct TeeWriter<'a, W: Write> {
    inner: &'a mut W,
    hasher: &'a mut Sha256,
}

impl<W: Write> Write for TeeWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
