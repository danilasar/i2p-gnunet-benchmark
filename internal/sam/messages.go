package sam

type MsgType string

const (
	MsgReady  MsgType = "ready"
	MsgResult MsgType = "result"
)

// ReadyMsg — первое сообщение receiver'а: SAM-сессия создана,
// STREAM ACCEPT вызван, готов к входящему соединению.
type ReadyMsg struct {
	Type MsgType `json:"type"`
	Dest string  `json:"dest"` // полный base64-ключ destination (I2PAddr), не b32
}

// ResultMsg — итоговые метрики. Поля с omitempty отсутствуют
// у той стороны, которая их не измеряет.
type ResultMsg struct {
	Type         MsgType `json:"type"`
	Role         string  `json:"role"`                    // "sender" | "receiver"
	SetupMs      float64 `json:"setup_ms,omitempty"`      // sender: от NewSAM до DialI2P вернул
	TransferMs   float64 `json:"transfer_ms"`             // время передачи payload
	FirstByteMs  float64 `json:"first_byte_ms,omitempty"` // receiver: от Accept до первого байта
	GoodputMbps  float64 `json:"goodput_mbps,omitempty"`  // sender: payload/transfer_ms
	PayloadBytes int64   `json:"payload_bytes"`
	SHA256OK     bool    `json:"sha256_ok"`
	Success      bool    `json:"success"`
	Error        string  `json:"error,omitempty"`
}
