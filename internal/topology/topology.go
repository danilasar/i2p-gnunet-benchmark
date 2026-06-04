package topology

import (
	"fmt"
	"os/exec"
	"strings"
	"testing"
)

type Topology struct {
	t      *testing.T
	Bridge string
	Nodes  []*Node
}

type Node struct {
	Index int
	NS    string // "ns_i2pN" или "ns_gnN"
	IP    string // "10.88.0.N+1"
	Veth  string // имя veth-интерфейса в namespace
}

func NewTopology(t *testing.T, n int, subnet string) *Topology {
	prefix := strings.ReplaceAll(subnet, ".", "_")
	bridgeName := fmt.Sprintf("br_%s", prefix)

	tp := &Topology{
		t:      t,
		Bridge: bridgeName,
		Nodes:  make([]*Node, n),
	}

	// cleanup registration
	t.Cleanup(func() {
		for i := n - 1; i >= 0; i-- {
			nsName := fmt.Sprintf("ns_%s%d", prefix, i)
			exec.Command("ip", "netns", "del", nsName).Run()
		}
		exec.Command("ip", "link", "set", bridgeName, "down").Run()
		exec.Command("ip", "link", "del", bridgeName, "type", "bridge").Run()
	})

	// 1. Create bridge
	tp.run("ip", "link", "add", bridgeName, "type", "bridge")
	tp.run("ip", "link", "set", bridgeName, "up")

	for i := 0; i < n; i++ {
		nsName := fmt.Sprintf("ns_%s%d", prefix, i)
		vethHost := fmt.Sprintf("veth_%s%d_h", prefix, i)
		vethNS := fmt.Sprintf("veth_%s%d_n", prefix, i)
		ipAddr := fmt.Sprintf("%s.%d/24", subnet, i+1)

		// 2. Create netns
		tp.run("ip", "netns", "add", nsName)
		tp.run("ip", "netns", "exec", nsName, "ip", "link", "set", "lo", "up")

		// 3. Create veth pair
		tp.run("ip", "link", "add", vethHost, "type", "veth", "peer", "name", vethNS)
		tp.run("ip", "link", "set", vethHost, "master", bridgeName)
		tp.run("ip", "link", "set", vethHost, "up")
		tp.run("ip", "link", "set", vethNS, "netns", nsName)

		// 4. Setup IP in NS
		tp.run("ip", "netns", "exec", nsName, "ip", "addr", "add", ipAddr, "dev", vethNS)
		tp.run("ip", "netns", "exec", nsName, "ip", "link", "set", vethNS, "up")

		tp.Nodes[i] = &Node{
			Index: i,
			NS:    nsName,
			IP:    fmt.Sprintf("%s.%d", subnet, i+1),
			Veth:  vethNS,
		}
	}

	return tp
}

func (tp *Topology) NodeIP(i int) string {
	if i < 0 || i >= len(tp.Nodes) {
		return ""
	}
	return tp.Nodes[i].IP
}

func (tp *Topology) run(name string, arg ...string) {
	cmd := exec.Command(name, arg...)
	if out, err := cmd.CombinedOutput(); err != nil {
		tp.t.Fatalf("command %s %v failed: %v\nOutput: %s", name, arg, err, string(out))
	}
}
