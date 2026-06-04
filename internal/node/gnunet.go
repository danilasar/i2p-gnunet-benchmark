package node

import (
	"bytes"
	"context"
	_ "embed"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"text/template"
	"time"
)

//go:embed gnunet.conf.tmpl
var gnunetConfTmpl string

type GnunetPeer struct {
	t        *testing.T
	Index    int
	DataDir  string
	ConfPath string
	NS       string
	IP       string
	Port     int
	cmd      *exec.Cmd
}

func NewGnunetPeer(t *testing.T, index int, ns, ip string, port int) *GnunetPeer {
	dataDir := t.TempDir()
	confPath := filepath.Join(dataDir, "peer.conf")

	p := &GnunetPeer{
		t:        t,
		Index:    index,
		DataDir:  dataDir,
		ConfPath: confPath,
		NS:       ns,
		IP:       ip,
		Port:     port,
	}

	// Create necessary directories
	dirs := []string{
		filepath.Join(dataDir, ".cache", "gnunet"),
		filepath.Join(dataDir, "data", "hosts"),
		filepath.Join(dataDir, "run"),
	}
	for _, d := range dirs {
		if err := os.MkdirAll(d, 0755); err != nil {
			t.Fatalf("failed to create dir %s: %v", d, err)
		}
	}

	t.Cleanup(func() {
		if p.cmd != nil {
			exec.Command("ip", "netns", "exec", p.NS, "gnunet-arm", "-c", p.ConfPath, "-e").Run()
			p.cmd.Wait()
		}
	})

	return p
}

func (p *GnunetPeer) WriteConfig() error {
	tmpl, err := template.New("gnunet").Parse(gnunetConfTmpl)
	if err != nil {
		return err
	}

	var buf bytes.Buffer
	err = tmpl.Execute(&buf, p)
	if err != nil {
		return err
	}

	return os.WriteFile(p.ConfPath, buf.Bytes(), 0644)
}

func (p *GnunetPeer) Start() error {
	p.cmd = exec.Command("ip", "netns", "exec", p.NS, "gnunet-arm", "-c", p.ConfPath, "-s", "-L", "DEBUG")
	if err := p.cmd.Start(); err != nil {
		return fmt.Errorf("failed to start gnunet-arm: %v", err)
	}
	return nil
}

func (p *GnunetPeer) ExportHello() (string, error) {
	cmd := exec.Command("ip", "netns", "exec", p.NS, "gnunet-hello", "-c", p.ConfPath, "-e")
	out, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to export hello: %v", err)
	}
	return strings.TrimSpace(string(out)), nil
}

func (p *GnunetPeer) ImportHello(hello string) error {
	cmd := exec.Command("ip", "netns", "exec", p.NS, "gnunet-hello", "-c", p.ConfPath, "--import")
	cmd.Stdin = strings.NewReader(hello)
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to import hello: %v, output: %s", err, string(out))
	}
	return nil
}

func (p *GnunetPeer) WaitCoreConnected(ctx context.Context) error {
	logFile := filepath.Join(p.DataDir, ".cache", "gnunet", "gnunet.log")
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			// 1. Try logs
			data, err := os.ReadFile(logFile)
			if err == nil {
				s := string(data)
				if strings.Contains(s, "notification about connection") ||
					strings.Contains(s, "connection established") ||
					strings.Contains(s, "connected to") {
					return nil
				}
			}

			// 2. Try gnunet-statistics
			cmd := exec.Command("ip", "netns", "exec", p.NS, "gnunet-statistics", "-c", p.ConfPath, "-q", "-s", "core")
			out, err := cmd.Output()
			if err == nil && len(out) > 0 {
				s := strings.ToLower(string(out))
				if strings.Contains(s, "neighbour") || strings.Contains(s, "connection") {
					return nil
				}
			}
		}
	}
}
