package sam

import (
	"crypto/sha256"
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"time"
)

// SendPayload пишет в conn: size-header + payload из reader + SHA256.
func SendPayload(conn net.Conn, r io.Reader, size int64) (int64, [32]byte, error) {
	// 1. Write size header (8 bytes big-endian)
	if err := binary.Write(conn, binary.BigEndian, uint64(size)); err != nil {
		return 0, [32]byte{}, fmt.Errorf("failed to write size: %v", err)
	}

	// 2. Write payload and calculate SHA256
	h := sha256.New()
	mw := io.MultiWriter(conn, h)
	written, err := io.CopyN(mw, r, size)
	if err != nil {
		return written, [32]byte{}, fmt.Errorf("failed to write payload: %v", err)
	}

	var sum [32]byte
	copy(sum[:], h.Sum(nil))

	// 3. Write SHA256 (32 bytes)
	if _, err := conn.Write(sum[:]); err != nil {
		return written, sum, fmt.Errorf("failed to write sha256: %v", err)
	}

	return written, sum, nil
}

// ReceivePayload читает из conn size-header, затем payload,
// затем 32 байта SHA256 от sender'а.
func ReceivePayload(conn net.Conn) (received int64, sha256ok bool, firstByteAt time.Time, err error) {
	// 1. Read size header
	var size uint64
	if err := binary.Read(conn, binary.BigEndian, &size); err != nil {
		return 0, false, time.Time{}, fmt.Errorf("failed to read size: %v", err)
	}

	// 2. Read first byte of payload to fix firstByteAt
	h := sha256.New()
	var buf [1]byte
	if _, err := io.ReadFull(conn, buf[:]); err != nil {
		return 0, false, time.Time{}, fmt.Errorf("failed to read first byte: %v", err)
	}
	firstByteAt = time.Now()
	h.Write(buf[:])

	// 3. Read rest of payload
	received = 1
	if size > 1 {
		rest := int64(size - 1)
		nr, err := io.CopyN(h, conn, rest)
		received += nr
		if err != nil {
			return received, false, firstByteAt, fmt.Errorf("failed to read payload: %v", err)
		}
	}

	// 4. Read SHA256 from sender
	var senderSum [32]byte
	if _, err := io.ReadFull(conn, senderSum[:]); err != nil {
		return received, false, firstByteAt, fmt.Errorf("failed to read sha256: %v", err)
	}

	// 5. Verify
	mySum := h.Sum(nil)
	sha256ok = true
	for i := 0; i < 32; i++ {
		if senderSum[i] != mySum[i] {
			sha256ok = false
			break
		}
	}

	return received, sha256ok, firstByteAt, nil
}
