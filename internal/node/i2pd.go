package node

import (
	"archive/zip"
	"bytes"
	"context"
	"crypto/sha256"
	_ "embed"
	"encoding/base64"
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
		n.Stop()
	})

	return n
}

type i2pdConfigData struct {
	Host       string
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
		Host:       n.IP,
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

func (n *I2pdNode) WaitRouterInfoFloodfill(ctx context.Context) error {
	if !n.Floodfill {
		return nil
	}
	ticker := time.NewTicker(1 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			data, err := os.ReadFile(n.RouterInfoPath())
			if err != nil {
				continue
			}
			if bytes.Contains(data, []byte("Xf")) {
				return nil
			}
		}
	}
}

// routerInfoI2PHash вычисляет SHA256(RouterIdentity) и возвращает
// строку в I2P base64 (+→-, /→~, без padding).
func routerInfoI2PHash(riPath string) (string, []byte, error) {
	data, err := os.ReadFile(riPath)
	if err != nil {
		return "", nil, fmt.Errorf("read router.info: %v", err)
	}
	if len(data) < 387 {
		return "", nil, fmt.Errorf("router.info too short")
	}
	certLen := int(data[385])<<8 | int(data[386])
	identityLen := 387 + certLen
	if len(data) < identityLen {
		return "", nil, fmt.Errorf("router.info too short for identity")
	}
	h := sha256.Sum256(data[:identityLen])
	b64 := base64.RawStdEncoding.EncodeToString(h[:])
	b64 = strings.ReplaceAll(b64, "+", "-")
	b64 = strings.ReplaceAll(b64, "/", "~")
	return b64, data, nil
}

func (n *I2pdNode) CreateReseedZip(dest string) error {
	return CreateMultiReseedZip(dest, []*I2pdNode{n})
}

// CreateMultiReseedZip создаёт ZIP с RouterInfo нескольких нод.
// Используется для cross-populate: i2pd сам раскладывает файлы по netDb.
func CreateMultiReseedZip(dest string, nodes []*I2pdNode) error {
	f, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer f.Close()
	archive := zip.NewWriter(f)
	defer archive.Close()
	for _, n := range nodes {
		b64, data, err := routerInfoI2PHash(n.RouterInfoPath())
		if err != nil {
			return err
		}
		entry, err := archive.Create(fmt.Sprintf("routerInfo-%s.dat", b64))
		if err != nil {
			return err
		}
		if _, err = entry.Write(data); err != nil {
			return err
		}
	}
	return nil
}

// InstallRouterInfo копирует router.info ноды src в netDb ноды dst
// с правильным именем файла (routerInfo-<hash>.dat).
func InstallRouterInfo(src, dst *I2pdNode) error {
	b64, data, err := routerInfoI2PHash(src.RouterInfoPath())
	if err != nil {
		return err
	}
	// i2pd ≥ 2.60 использует схему netDb/r<первый_char_hash>/routerInfo-<hash>.dat
	subdir := filepath.Join(dst.DataDir, "netDb", "r"+b64[:1])
	if err := os.MkdirAll(subdir, 0755); err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(subdir, "routerInfo-"+b64+".dat"), data, 0644)
}

// Stop убивает все процессы i2pd с этим datadir и ждёт их реальной гибели.
// i2pd --daemon форкает: PID-файл может содержать PID родителя (который уже умер),
// а не демона. Используем pkill -9 (SIGKILL) + pgrep чтобы убедиться что всё мертво.
func (n *I2pdNode) Stop() {
	exec.Command("pkill", "-9", "-f", "datadir="+n.DataDir).Run()
	// Ждём пока pgrep перестанет находить процессы
	for range 50 {
		time.Sleep(200 * time.Millisecond)
		out, _ := exec.Command("pgrep", "-f", "datadir="+n.DataDir).Output()
		if len(strings.TrimSpace(string(out))) == 0 {
			break
		}
	}
	// Убираем стale PID-файл чтобы следующий запуск не путался
	os.Remove(filepath.Join(n.DataDir, "i2pd.pid"))
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
