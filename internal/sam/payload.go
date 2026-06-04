package sam

import (
	"crypto/sha256"
	"io"
)

type payloadReader struct {
	size int64
	seed int64
	pos  int64
}

func (r *payloadReader) Read(p []byte) (n int, err error) {
	if r.pos >= r.size {
		return 0, io.EOF
	}
	for i := 0; i < len(p); i++ {
		if r.pos >= r.size {
			return i, nil
		}
		// Formula: byte = (i*seed + i) % 256 as per requirement
		// Actually i here should probably be the absolute position r.pos
		p[i] = byte((r.pos*r.seed + r.pos) % 256)
		r.pos++
	}
	return len(p), nil
}

// NewPayloadReader возвращает io.Reader, генерирующий size байт
// детерминированно из seed.
func NewPayloadReader(size int64, seed int64) io.Reader {
	return &payloadReader{
		size: size,
		seed: seed,
		pos:  0,
	}
}

// ExpectedSHA256 возвращает SHA256 payload, сгенерированного
// из тех же size и seed.
func ExpectedSHA256(size int64, seed int64) [32]byte {
	h := sha256.New()
	r := NewPayloadReader(size, seed)
	io.Copy(h, r)
	var sum [32]byte
	copy(sum[:], h.Sum(nil))
	return sum
}
