package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"log"
	"os"
	"time"

	"github.com/go-i2p/i2pkeys"
	"github.com/go-i2p/sam3"
)

type ReadyMsg struct {
	Type string `json:"type"`
	Dest string `json:"dest"`
}

type ResultMsg struct {
	Type    string `json:"type"`
	Success bool   `json:"success"`
	Error   string `json:"error"`
}

func main() {
	role := flag.String("role", "", "role: server or client")
	samAddr := flag.String("sam", "127.0.0.1:7656", "SAM address")
	id := flag.String("id", "go-peer", "session ID")
	dest := flag.String("dest", "", "destination to connect to (client only)")
	msg := flag.String("msg", "", "message to send (client only)")
	timeout := flag.Duration("timeout", 120*time.Second, "total timeout")
	flag.Parse()

	if *role == "" {
		log.Fatal("--role is required")
	}

	err := run(*role, *samAddr, *id, *dest, *msg, *timeout)
	if err != nil {
		outputResult(false, err.Error())
		os.Exit(1)
	}
}

func run(role, samAddr, id, dest, msg string, timeout time.Duration) error {
	sam, err := sam3.NewSAM(samAddr)
	if err != nil {
		return fmt.Errorf("NewSAM: %w", err)
	}
	defer sam.Close()

	keys, err := sam.NewKeys()
	if err != nil {
		return fmt.Errorf("NewKeys: %w", err)
	}

	// Use empty options for simplicity
	session, err := sam.NewStreamSession(id, keys, []string{})
	if err != nil {
		return fmt.Errorf("NewStreamSession: %w", err)
	}
	defer session.Close()

	if role == "server" {
		return runServer(session, keys)
	} else if role == "client" {
		return runClient(session, dest, msg)
	} else {
		return fmt.Errorf("invalid role: %s", role)
	}
}

func runServer(session *sam3.StreamSession, keys i2pkeys.I2PKeys) error {
	outputReady(keys.Addr().Base64())

	listener, err := session.Listen()
	if err != nil {
		return fmt.Errorf("Listen: %w", err)
	}
	defer listener.Close()

	conn, err := listener.Accept()
	if err != nil {
		return fmt.Errorf("Accept: %w", err)
	}
	defer conn.Close()

	// Echo back everything
	_, err = io.Copy(conn, conn)
	if err != nil && err != io.EOF {
		return fmt.Errorf("io.Copy: %w", err)
	}

	outputResult(true, "")
	return nil
}

func runClient(session *sam3.StreamSession, dest, msg string) error {
	i2pDest := i2pkeys.I2PAddr(dest)
	conn, err := session.DialI2P(i2pDest)
	if err != nil {
		return fmt.Errorf("DialI2P: %w", err)
	}
	defer conn.Close()

	_, err = conn.Write([]byte(msg))
	if err != nil {
		return fmt.Errorf("Write: %w", err)
	}

	// Read back echo
	buf := make([]byte, len(msg))
	_, err = io.ReadFull(conn, buf)
	if err != nil {
		return fmt.Errorf("ReadFull: %w", err)
	}

	if string(buf) != msg {
		return fmt.Errorf("echo mismatch: expected %q, got %q", msg, string(buf))
	}

	outputResult(true, "")
	return nil
}

func outputReady(dest string) {
	json.NewEncoder(os.Stdout).Encode(ReadyMsg{
		Type: "ready",
		Dest: dest,
	})
}

func outputResult(success bool, errMsg string) {
	json.NewEncoder(os.Stdout).Encode(ResultMsg{
		Type:    "result",
		Success: success,
		Error:   errMsg,
	})
}
