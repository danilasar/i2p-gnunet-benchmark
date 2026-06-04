# Руководство разработчика

## Архитектура

```
┌─────────────────────────────────────────────┐
│  testbed/ (go test)                          │
│  smoke_test.go, pilot_test.go, ...           │
└──────────────┬──────────────────────────────┘
               │ использует
┌──────────────▼──────────────────────────────┐
│  internal/                                   │
│  ├── topology/   netns, veth, bridge         │
│  ├── node/       GnunetPeer, I2pdNode        │
│  ├── config/     генератор конфигов (TODO)   │
│  └── metrics/    CPU/RSS, JSONL лог (TODO)   │
└──────────────┬──────────────────────────────┘
               │ запускает субпроцессы
┌──────────────▼──────────────────────────────┐
│  Docker-контейнер (alt:sisyphus)             │
│  gnunet-arm, i2pd, gnunet-communicator-tcp   │
│  rs/sam-sender, rs/sam-receiver  (Rust)      │
└─────────────────────────────────────────────┘
```

Все тесты запускаются внутри `--privileged` контейнера.
Go-код управляет нодами через `os/exec` и `ip netns exec`.

## Пакеты

### `internal/topology`

Создаёт Linux network namespace, veth-пару и bridge для каждой ноды.
Cleanup регистрируется через `t.Cleanup()` в обратном порядке.

```go
tp := topology.NewTopology(t, n, "10.88.0")
// tp.Nodes[i].NS  — имя namespace
// tp.Nodes[i].IP  — IP-адрес
// tp.Nodes[i].Veth — имя veth внутри namespace
```

Все ошибки `ip`-команд — `t.Fatalf`. Топология не частично создаётся.

### `internal/node`

**`GnunetPeer`** — один GNUnet-пир:

```go
p := node.NewGnunetPeer(t, index, ns, ip, port)
p.WriteConfig()                          // пишет peer.conf из шаблона
p.Start()                                // gnunet-arm -s (неблокирующий)
hello, _ := p.ExportHello()             // gnunet-hello -e
p.ImportHello(hello)                    // gnunet-hello --import
p.WaitCoreConnected(ctx)                // поллит лог, ищет "notification about connection from"
```

**`I2pdNode`** — один i2pd-роутер:

```go
n := node.NewI2pdNode(t, index, ns, ip, port, floodfill)
n.SAMPort = 17656                        // 0 = SAM отключён
n.WriteConfig(zipFile)                  // пишет i2pd.conf из шаблона
n.Start()                               // i2pd --daemon (неблокирующий)
n.CreateReseedZip(dest)                 // router.info → ZIP с правильным именем
n.WaitBootstrapped(ctx)                 // поллит лог, ищет "NetDbReq: Exploring new"
```

Конфиги генерируются из `*.conf.tmpl` через `text/template`, встроенных через `//go:embed`.

### `internal/config` (TODO)

Планируется: генератор underlay-матрицы (netem-профили P1–P7), параметров туннелей.

### `internal/metrics` (TODO)

Планируется: сбор CPU/RSS из `/proc/<pid>/status`, запись JSONL.

## Конвенции

**Управление ресурсами:** всегда `t.Cleanup()`, никогда `defer` на уровне теста.
`t.TempDir()` для всех временных директорий — очищается автоматически.

**Ошибки:** в `internal/` методы возвращают `error`; в `testbed/` используется
`require.NoError(t, err)` — тест сразу останавливается при первой ошибке.

**Ожидание:** `context.WithTimeout` + поллинг лога раз в секунду.
Никаких `time.Sleep` без явного обоснования в комментарии.

**Субпроцессы:** только `os/exec`, никаких CGo и прямых syscall.
`gnunet-arm -s` — через `cmd.Start()` (неблокирующий).
`i2pd --daemon` — через `cmd.CombinedOutput()` (daemon форкается сам).

## Добавить новый сценарий

1. Создать файл `testbed/<name>_test.go` с функцией `Test<Name>(t *testing.T)`.
2. Использовать `topology.NewTopology` и типы из `internal/node`.
3. Зарегистрировать в `Justfile` отдельную команду `just test-<name>`.

Пример минимального теста:

```go
func TestNewScenario(t *testing.T) {
    tp := topology.NewTopology(t, 2, "10.77.0")

    peer := node.NewGnunetPeer(t, 0, tp.Nodes[0].NS, tp.Nodes[0].IP, 2200)
    require.NoError(t, peer.WriteConfig())
    require.NoError(t, peer.Start())

    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()
    require.NoError(t, peer.WaitCoreConnected(ctx))
}
```

## Добавить новый тип ноды

1. Создать `internal/node/<name>.go` с типом и методами.
2. Создать `internal/node/<name>.conf.tmpl` — конфиг-шаблон.
3. Embed шаблон через `//go:embed <name>.conf.tmpl`.
4. Реализовать минимальный интерфейс: `WriteConfig`, `Start`, `Wait<Condition>`.

## Rust: sam-sender / sam-receiver

Сборка:
```bash
just build-rs          # cargo build в rs/
```

Бинари появляются в `rs/target/debug/`. Go-тесты вызывают их через `exec.Command`.
Путь к бинарю: `rs/target/debug/sam-sender` (или `release` в CI).

Интерфейс (планируется):
```
sam-sender  --sam <addr:port> --dest <b32> --size <bytes> --seed <n> --out <jsonl>
sam-receiver --sam <addr:port> --out <jsonl>
```

## Docker и CI

**Образ** (`Dockerfile`):
- Базовый: `alt:sisyphus`
- Слой 1: `apt-get install` — gnunet, i2pd, golang, gcc, iputils
- Слой 2: rustup + `$PATH`
- Слой 3: `COPY rs/` + `cargo build`

Пересборка нужна только при изменении `Dockerfile` или `rs/`.
Изменения в `internal/` и `testbed/` подхватываются через volume mount.

**CI** (`.github/workflows/ci.yml`):
- Триггер: push в `master`/`main`, PR
- Шаги: checkout → build image → `go test -run TestSmoke -timeout 5m`

## I2P base64

I2P использует нестандартный base64: `+`→`-`, `/`→`~`, без padding.
В Go: `base64.RawStdEncoding.EncodeToString(hash)` + ручная замена.
**Не использовать** `base64.RawURLEncoding` — он даёт `_` вместо `~`.

## Известные ограничения

- `t.TempDir()` создаёт директории на хосте, но монтируется в контейнер через `-v`.
  При запуске вне контейнера пути могут не совпасть.
- `gnunet-arm` требует `GNUNET_HOME` = dataDir при вызове вспомогательных команд
  (`gnunet-hello`, `gnunet-statistics`) — это задаётся через `GNUNET_HOME` в env или через `-c`.
- i2pd bootstrap занимает ~55 с — таймауты в тестах должны быть ≥ 90 с.
