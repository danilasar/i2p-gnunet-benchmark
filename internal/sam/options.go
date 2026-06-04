package sam

func streamTunnelOptions() []string {
	return []string{
		"inbound.length=0",
		"outbound.length=0",
		"inbound.lengthVariance=0",
		"outbound.lengthVariance=0",
		"inbound.backupQuantity=0",
		"outbound.backupQuantity=0",
		"inbound.quantity=2",
		"outbound.quantity=2",
	}
}
