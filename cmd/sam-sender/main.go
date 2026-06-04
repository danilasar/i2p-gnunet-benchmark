package main

import (
	"coursework/testbed/internal/sam"
	"flag"
	"fmt"
	"os"
	"time"

	"github.com/sirupsen/logrus"
)

func main() {
	logrus.SetOutput(os.Stderr)
	logrus.SetLevel(logrus.WarnLevel)

	samAddr := flag.String("sam", "127.0.0.1:7656", "SAM bridge address")
	dest := flag.String("dest", "", "receiver destination (full base64 I2P address)")
	size := flag.Int64("size", 0, "payload size in bytes")
	seed := flag.Int64("seed", 42, "payload seed")
	id := flag.String("id", fmt.Sprintf("sender_%d", time.Now().UnixNano()), "session ID")
	timeout := flag.Int("timeout", 120, "timeout in seconds")

	flag.Parse()

	if *dest == "" || *size == 0 {
		flag.Usage()
		os.Exit(1)
	}

	cfg := sam.SenderConfig{
		SAMAddr: *samAddr,
		Dest:    *dest,
		Size:    *size,
		Seed:    *seed,
		ID:      *id,
		Timeout: time.Duration(*timeout) * time.Second,
	}

	if err := sam.RunSender(cfg, os.Stdout); err != nil {
		os.Exit(1)
	}
}
