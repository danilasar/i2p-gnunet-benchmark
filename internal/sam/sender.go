package sam

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"time"

	"github.com/go-i2p/i2pkeys"
	"github.com/go-i2p/sam3"
)

type SenderConfig struct {
	SAMAddr string
	Dest    string
	Size    int64
	Seed    int64
	ID      string
	Timeout time.Duration
}

func RunSender(cfg SenderConfig, w io.Writer) error {
	res := ResultMsg{
		Type:    MsgResult,
		Role:    "sender",
		Success: false,
	}
	enc := json.NewEncoder(w)

	fail := func(format string, args ...any) error {
		res.Error = fmt.Sprintf(format, args...)
		enc.Encode(res)
		return errors.New(res.Error)
	}

	t0 := time.Now()
	deadline := t0.Add(cfg.Timeout)

	s, err := sam3.NewSAM(cfg.SAMAddr)
	if err != nil {
		return fail("SAM connect: %v", err)
	}
	defer s.Close()

	keys, err := s.NewKeys()
	if err != nil {
		return fail("NewKeys: %v", err)
	}

	session, err := s.NewStreamSession(cfg.ID, keys, streamTunnelOptions())
	if err != nil {
		return fail("NewStreamSession: %v", err)
	}
	defer session.Close()

	destAddr := i2pkeys.I2PAddr(cfg.Dest)

	// Dial с повторными попытками до исчерпания таймаута.
	// DialI2P может вернуть ошибку если LeaseSet ещё не распространился.
	// DialI2P блокируется на conn.Read без deadline — оборачиваем в горутину.
	// Если попытка зависает дольше perAttempt, получаем timeout и идём на retry.
	// Горутина утекает до момента когда i2pd закроет TCP-соединение.
	dialOnce := func(perAttempt time.Duration) (net.Conn, error) {
		type dialRes struct {
			conn net.Conn
			err  error
		}
		ch := make(chan dialRes, 1)
		go func() {
			c, e := session.DialI2P(destAddr)
			ch <- dialRes{c, e}
		}()
		select {
		case r := <-ch:
			return r.conn, r.err
		case <-time.After(perAttempt):
			return nil, fmt.Errorf("DialI2P per-attempt timeout (%v)", perAttempt)
		}
	}

	const perAttempt = 90 * time.Second
	retryInterval := 5 * time.Second
	var conn net.Conn
	for {
		conn, err = dialOnce(perAttempt)
		if err == nil {
			break
		}
		if time.Now().Add(retryInterval).After(deadline) {
			return fail("DialI2P: %v", err)
		}
		time.Sleep(retryInterval)
	}
	defer conn.Close()

	t1 := time.Now()
	res.SetupMs = float64(t1.Sub(t0).Milliseconds())

	r := NewPayloadReader(cfg.Size, cfg.Seed)
	written, _, err := SendPayload(conn, r, cfg.Size)
	t2 := time.Now()

	res.PayloadBytes = written
	if err != nil {
		return fail("SendPayload: %v", err)
	}
	time.Sleep(5 * time.Second)

	res.Success = true
	res.TransferMs = float64(t2.Sub(t1).Milliseconds())
	if res.TransferMs > 0 {
		res.GoodputMbps = (float64(written) * 8 / 1e6) / (res.TransferMs / 1000)
	}
	res.SHA256OK = true

	return enc.Encode(res)
}
