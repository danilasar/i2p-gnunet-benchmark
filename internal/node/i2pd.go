package node

import (
	"archive/zip"
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
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

//go:embed i2pd.conf.tmpl
var i2pdConfTmpl string

type I2pdNode struct {
	t         *testing.T
	Index     int
	DataDir   string
	ConfPath  string
	NS        string
	IP        string
	Port      int
	Floodfill bool
	SAMPort   int // 0 = SAM отключён
	cmd       *exec.Cmd
}

func NewI2pdNode(t *testing.T, index int, ns, ip string, port int, floodfill bool) *I2pdNode {
	dataDir := t.TempDir()
	confPath := filepath.Join(dataDir, "i2pd.conf")

	n := &I2pdNode{
		t:         t,
		Index:     index,
		DataDir:   dataDir,
		ConfPath:  confPath,
		NS:        ns,
		IP:        ip,
		Port:      port,
		Floodfill: floodfill,
	}

	if err := os.MkdirAll(filepath.Join(dataDir, "netDb"), 0755); err != nil {
		t.Fatalf("failed to create netDb dir: %v", err)
	}

	t.Cleanup(func() {
		exec.Command("pkill", "-f", fmt.Sprintf("datadir=%s ", n.DataDir)).Run()
		time.Sleep(2 * time.Second)
	})

	return n
}

type i2pdConfigData struct {
	ZipFile    string
	Port       int
	SAMEnabled bool
	SAMPort    int
}

func (n *I2pdNode) WriteConfig(zipFile string) error {
	tmpl, err := template.New("i2pd").Parse(i2pdConfTmpl)
	if err != nil {
		return err
	}

	data := i2pdConfigData{
		ZipFile:    zipFile,
		Port:       n.Port,
		SAMEnabled: n.SAMPort > 0,
		SAMPort:    n.SAMPort,
	}

	var buf bytes.Buffer
	err = tmpl.Execute(&buf, data)
	if err != nil {
		return err
	}

	return os.WriteFile(n.ConfPath, buf.Bytes(), 0644)
}

func (n *I2pdNode) Start() error {
	args := []string{"netns", "exec", n.NS, "i2pd",
		"--datadir=" + n.DataDir,
		"--conf=" + n.ConfPath,
		"--address4=" + n.IP,
		"--loglevel=info",
		"--log=file",
		"--logfile=" + filepath.Join(n.DataDir, "i2pd.log"),
		"--daemon",
	}
	if n.Floodfill {
		args = append(args, "--floodfill")
	}

	cmd := exec.Command("ip", args...)
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to start i2pd: %v, output: %s", err, string(out))
	}
	return nil
}

func (n *I2pdNode) RouterInfoPath() string {
	return filepath.Join(n.DataDir, "router.info")
}

func (n *I2pdNode) CreateReseedZip(dest string) error {
	riPath := n.RouterInfoPath()
	data, err := os.ReadFile(riPath)
	if err != nil {
		return fmt.Errorf("failed to read router.info: %v", err)
	}

	if len(data) < 387 {
		return fmt.Errorf("router.info too short")
	}
	certLen := int(data[385])<<8 | int(data[386])
	identityLen := 387 + certLen
	if len(data) < identityLen {
		return fmt.Errorf("router.info too short for identity")
	}
	identity := data[:identityLen]

	hash := sha256.Sum256(identity)
	// Base64 RawStdEncoding with manual replacement as per requirement
	b64 := base64.RawStdEncoding.EncodeToString(hash[:])
	b64 = strings.ReplaceAll(b64, "+", "-")
	b64 = strings.ReplaceAll(b64, "/", "~")
	
	fileName := fmt.Sprintf("routerInfo-%s.dat", b64)

	zipFile, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer zipFile.Close()

	archive := zip.NewWriter(zipFile)
	defer archive.Close()

	f, err := archive.Create(fileName)
	if err != nil {
		return err
	}
	_, err = f.Write(data)
	return err
}

func (n *I2pdNode) WaitBootstrapped(ctx context.Context) error {
	logFile := filepath.Join(n.DataDir, "i2pd.log")
	ticker := time.NewTicker(1 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			data, err := os.ReadFile(logFile)
			if err != nil {
				continue
			}
			if strings.Contains(string(data), "NetDbReq: Exploring new") || strings.Contains(string(data), "Tunnel: all tunnels built") {
				return nil
			}
		}
	}
}
