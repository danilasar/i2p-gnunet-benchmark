package testbed

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"coursework/testbed/internal/node"
	"coursework/testbed/internal/sam"
	"coursework/testbed/internal/topology"

	"github.com/stretchr/testify/require"
)

func TestSAMTransfer(t *testing.T) {
	tp := topology.NewTopology(t, 4, "10.88.0")
	nodes := make([]*node.I2pdNode, 4)

	// --- Фаза 1: генерация RouterInfo с реальными соединениями ---
	// Node0 (floodfill) стартует один → генерирует router.info с caps=Xf.
	// Nodes 1-3 стартуют с zipPhase1 → соединяются с node0, обмениваются RouterInfo.
	// Это гарантирует актуальный RouterInfo с правильными caps в финальном ZIP.
	t.Log("Phase 1: generating RouterInfos...")
	for i := 0; i < 4; i++ {
		nodes[i] = node.NewI2pdNode(t, i, tp.Nodes[i].NS, tp.Nodes[i].IP, 12000+i, i == 0)
	}
	require.NoError(t, nodes[0].WriteConfig(""))
	require.NoError(t, nodes[0].Start())
	ffCtx, cancelFF := context.WithTimeout(context.Background(), 90*time.Second)
	require.NoError(t, nodes[0].WaitRouterInfoFloodfill(ffCtx))
	cancelFF()
	time.Sleep(5 * time.Second)

	zipPhase1 := filepath.Join(t.TempDir(), "reseed_phase1.zip")
	require.NoError(t, nodes[0].CreateReseedZip(zipPhase1))
	for i := 1; i < 4; i++ {
		require.NoError(t, nodes[i].WriteConfig(zipPhase1))
		require.NoError(t, nodes[i].Start())
	}
	time.Sleep(15 * time.Second) // ждём соединений и обмена RouterInfo

	for i := 0; i < 4; i++ {
		if _, err := os.Stat(nodes[i].RouterInfoPath()); err != nil {
			t.Fatalf("node%d did not generate router.info: %v", i, err)
		}
	}

	t.Log("Phase 1: stopping nodes...")
	for i := 0; i < 4; i++ {
		nodes[i].Stop()
	}
	time.Sleep(3 * time.Second)

	// Создаём единый ZIP со всеми четырьмя RouterInfo (обновлёнными после соединений).
	t.Log("Creating full reseed ZIP...")
	fullZip := filepath.Join(t.TempDir(), "reseed_full.zip")
	require.NoError(t, node.CreateMultiReseedZip(fullZip, nodes[:]))

	// Удаляем лог-файлы Phase 1 чтобы WaitBootstrapped не ложно срабатывал
	// по Phase 1 записям ("NetDbReq: Exploring new" из Phase 1 остаётся в файле).
	for i := 0; i < 4; i++ {
		os.Remove(filepath.Join(nodes[i].DataDir, "i2pd.log"))
	}

	// --- Фаза 2: основной запуск с SAM ---
	// node0 (floodfill) стартует БЕЗ ZIP: i2pd 2.60 пропускает reseed для floodfill-нод.
	// Nodes 1-3 получают fullZip → находят node0 как floodfill → соединяются.
	t.Log("Phase 2: starting nodes...")

	// node0: floodfill, без SAM, без ZIP
	require.NoError(t, nodes[0].WriteConfig(""))
	require.NoError(t, nodes[0].Start())

	// node1: receiver
	nodes[1].SAMPort = 7656
	require.NoError(t, nodes[1].WriteConfig(fullZip))
	require.NoError(t, nodes[1].Start())

	// node2: sender
	nodes[2].SAMPort = 7656
	require.NoError(t, nodes[2].WriteConfig(fullZip))
	require.NoError(t, nodes[2].Start())

	// node3: фоновый relay
	require.NoError(t, nodes[3].WriteConfig(fullZip))
	require.NoError(t, nodes[3].Start())

	bootCtx, cancelBoot := context.WithTimeout(context.Background(), 4*time.Minute)
	defer cancelBoot()

	t.Log("Waiting for nodes to bootstrap...")
	for i := 1; i < 4; i++ {
		require.NoError(t, nodes[i].WaitBootstrapped(bootCtx), "node %d bootstrap failed", i)
	}
	cancelBoot()

	// Отдельный контекст для фазы передачи — bootstrap не съедает его время.
	transferCtx, cancelTransfer := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancelTransfer()

	const (
		payloadSize = 1 * 1024 * 1024 // 1 MiB
		seed        = 42
		timeout     = "300" // 5 мин < 6 мин transferCtx; даёт место для нескольких retry DialI2P
	)
	runID := fmt.Sprintf("transfer_%d", time.Now().UnixNano())

	// Запускаем receiver в ns_i2p1
	t.Log("Starting receiver...")
	recvCmd := exec.CommandContext(transferCtx, "ip", "netns", "exec",
		tp.Nodes[1].NS, "sam-receiver",
		"--sam", "127.0.0.1:7656",
		"--size", strconv.Itoa(payloadSize),
		"--seed", strconv.Itoa(seed),
		"--id", "recv-"+runID,
		"--timeout", timeout,
	)
	recvStdout, err := recvCmd.StdoutPipe()
	require.NoError(t, err)
	var recvStderr bytes.Buffer
	recvCmd.Stderr = &recvStderr
	require.NoError(t, recvCmd.Start())

	// Ждём ReadyMsg (STREAM ACCEPT вызван)
	scanner := bufio.NewScanner(recvStdout)
	require.True(t, scanWithTimeout(t, scanner, 2*time.Minute, func() {
		t.Logf("sam-receiver stderr:\n%s", recvStderr.String())
		printNodeLogs(t, nodes[:])
	}), "receiver did not send ready message")
	var ready sam.ReadyMsg
	require.NoError(t, json.Unmarshal(scanner.Bytes(), &ready))
	require.Equal(t, sam.MsgReady, ready.Type)
	t.Logf("receiver ready, dest=%.20s...", ready.Dest)

	// Запускаем sender в ns_i2p2
	t.Log("Starting sender...")
	senderCmd := exec.CommandContext(transferCtx, "ip", "netns", "exec",
		tp.Nodes[2].NS, "sam-sender",
		"--sam", "127.0.0.1:7656",
		"--dest", ready.Dest,
		"--size", strconv.Itoa(payloadSize),
		"--seed", strconv.Itoa(seed),
		"--id", "send-"+runID,
		"--timeout", timeout,
	)
	var senderStderr bytes.Buffer
	senderCmd.Stderr = &senderStderr
	senderOut, err := senderCmd.Output()
	if err != nil {
		t.Logf("sam-sender stderr:\n%s", senderStderr.String())
		t.Logf("sam-sender stdout:\n%s", senderOut)
		// проверяем сетевую связность между node1 (10.88.0.2) и node2 (10.88.0.3)
		out, _ := exec.Command("ip", "netns", "exec", tp.Nodes[2].NS,
			"bash", "-c", fmt.Sprintf("echo x | nc -q1 -w2 %s 12001 && echo REACHABLE || echo UNREACHABLE", tp.Nodes[1].IP)).CombinedOutput()
		t.Logf("node2->node1 NTCP2 port check: %s", strings.TrimSpace(string(out)))
		printNodeLogs(t, nodes[:])
		require.NoError(t, err, "sam-sender failed")
	}

	var sendResult sam.ResultMsg
	require.NoError(t, json.Unmarshal(bytes.TrimSpace(senderOut), &sendResult))

	// Читаем ResultMsg от receiver'а
	require.True(t, scanWithTimeout(t, scanner, 2*time.Minute, func() {
		t.Logf("sam-receiver stderr:\n%s", recvStderr.String())
		printNodeLogs(t, nodes[:])
	}), "receiver did not send result message")
	var recvResult sam.ResultMsg
	require.NoError(t, json.Unmarshal(scanner.Bytes(), &recvResult))
	recvWaitErr := recvCmd.Wait()

	if !sendResult.Success || !recvResult.Success || !recvResult.SHA256OK {
		t.Logf("sender result: %+v", sendResult)
		t.Logf("receiver result: %+v", recvResult)
		t.Logf("sam-receiver stderr:\n%s", recvStderr.String())
		printNodeLogs(t, nodes[:])
	}
	if recvResult.Success {
		require.NoError(t, recvWaitErr)
	}
	require.True(t, sendResult.Success, "sender error: %s", sendResult.Error)
	require.True(t, recvResult.Success, "receiver error: %s", recvResult.Error)
	require.True(t, recvResult.SHA256OK, "sha256 mismatch")
	require.Equal(t, int64(payloadSize), recvResult.PayloadBytes)

	t.Logf("setup=%.0fms transfer=%.0fms goodput=%.2f Mbps first_byte=%.0fms",
		sendResult.SetupMs, sendResult.TransferMs,
		sendResult.GoodputMbps, recvResult.FirstByteMs)
}

func scanWithTimeout(t *testing.T, scanner *bufio.Scanner, timeout time.Duration, onTimeout func()) bool {
	t.Helper()
	done := make(chan bool, 1)
	go func() {
		done <- scanner.Scan()
	}()
	select {
	case ok := <-done:
		return ok
	case <-time.After(timeout):
		onTimeout()
		return false
	}
}

func printNodeLogs(t *testing.T, nodes []*node.I2pdNode) {
	t.Helper()
	artifactDir := os.Getenv("TEST_ARTIFACT_DIR")
	for i, n := range nodes {
		logPath := filepath.Join(n.DataDir, "i2pd.log")
		data, err := os.ReadFile(logPath)
		if err != nil {
			continue
		}
		if artifactDir != "" {
			if err := os.MkdirAll(artifactDir, 0755); err == nil {
				dst := filepath.Join(artifactDir, fmt.Sprintf("node%d-i2pd.log", i))
				if err := os.WriteFile(dst, data, 0644); err == nil {
					t.Logf("saved full node%d log to %s", i, dst)
				}
			}
		}
		// Печатаем только строки, релевантные для диагностики SAM/LeaseSet
		var lines []string
		for _, line := range strings.Split(string(data), "\n") {
			if strings.ContainsAny(line, "") {
				continue
			}
			for _, kw := range []string{"SAM", "LeaseSet", "floodfill", "Exploring", "routers loaded",
				"Reseed", "NTCP2: Connected", "NTCP2: Established", "transport", "error"} {
				if strings.Contains(line, kw) {
					lines = append(lines, line)
					break
				}
			}
		}
		t.Logf("=== node%d relevant log (%d lines) ===\n%s", i, len(lines), strings.Join(lines, "\n"))
	}
}
