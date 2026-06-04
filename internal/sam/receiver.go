package sam

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"strings"
	"time"

	"github.com/go-i2p/sam3"
)

type ReceiverConfig struct {
	SAMAddr string
	Size    int64
	Seed    int64
	ID      string
	Timeout time.Duration
}

func RunReceiver(cfg ReceiverConfig, w io.Writer) error {
	ctx, cancel := context.WithTimeout(context.Background(), cfg.Timeout)
	defer cancel()

	res := ResultMsg{
		Type:    MsgResult,
		Role:    "receiver",
		Success: false,
	}
	enc := json.NewEncoder(w)

	fail := func(format string, args ...any) error {
		res.Error = fmt.Sprintf(format, args...)
		enc.Encode(res)
		return errors.New(res.Error)
	}

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

	listener, err := session.Listen()
	if err != nil {
		return fail("Listen: %v", err)
	}
	defer listener.Close()

	// Публикуем полный base64-ключ destination, а не только b32-хэш.
	// DialI2P на sender-стороне принимает i2pkeys.I2PAddr (полный ключ).
	if err := enc.Encode(ReadyMsg{Type: MsgReady, Dest: string(keys.Addr())}); err != nil {
		return err
	}

	type acceptResult struct {
		conn net.Conn
		err  error
	}

	for {
		tAccept := time.Now()
		ch := make(chan acceptResult, 1)
		go func() {
			c, e := listener.Accept()
			ch <- acceptResult{c, e}
		}()

		var conn net.Conn
		select {
		case <-ctx.Done():
			return fail("timeout waiting for connection")
		case r := <-ch:
			if r.err != nil {
				return fail("Accept: %v", r.err)
			}
			conn = r.conn
		}

		received, shaOk, firstByteAt, err := ReceivePayload(conn)
		conn.Close()
		tDone := time.Now()

		// i2pd/SAM can occasionally deliver a reset/empty accepted stream before
		// the real payload stream. Ignore it and keep accepting until timeout.
		if err != nil && received == 0 && strings.Contains(err.Error(), "failed to read size: EOF") {
			continue
		}

		res.PayloadBytes = received
		res.SHA256OK = shaOk
		if err != nil {
			return fail("ReceivePayload: %v", err)
		}

		res.Success = true
		res.FirstByteMs = float64(firstByteAt.Sub(tAccept).Milliseconds())
		res.TransferMs = float64(tDone.Sub(firstByteAt).Milliseconds())

		return enc.Encode(res)
	}
}
