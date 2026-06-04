# Руководство разработчика

## Архитектура

```
testbed/smoke_test.go
    └── internal/topology   — netns, veth, bridge
    └── internal/node       — GnunetPeer, I2pdNode
        └── *.conf.tmpl     — конфиги через text/template + go:embed

Всё запускается внутри Docker (--privileged).
Go-код управляет нодами через os/exec и ip netns exec.
```

Пакеты `internal/config` и `internal/metrics` запланированы, но не реализованы.

## `internal/topology`

```go
tp := topology.NewTopology(t, n, "10.88.0")
tp.Nodes[i].NS    // имя namespace: "ns_10_88_0<i>"
tp.Nodes[i].IP    // "10.88.0.<i+1>"
tp.Nodes[i].Veth  // имя veth внутри namespace
```

Создаёт bridge, N namespace, N veth-пар. Cleanup — `t.Cleanup()` в обратном
порядке. Любая ошибка `ip`-команды — `t.Fatalf`.

## `internal/node`

### GnunetPeer

```go
p := node.NewGnunetPeer(t, index, ns, ip, port)
p.WriteConfig()                   // peer.conf из gnunet.conf.tmpl
p.Start()                         // gnunet-arm -s, cmd.Start() (неблокирующий)
hello, _ := p.ExportHello()      // gnunet-hello -e
p.ImportHello(hello)             // gnunet-hello --import
p.WaitCoreConnected(ctx)         // поллит лог, ищет "notification about connection from"
```

Cleanup: `gnunet-arm -e` + `cmd.Wait()`.

Конфиг (`gnunet.conf.tmpl`): `@INLINE@` стандартных конфигов из
`/usr/share/gnunet/config.d/`, override для `[PATHS]`, `[communicator-tcp]`,
`[communicator-udp]` (отключён), `[hostlist]` (SERVERS =).

### I2pdNode

```go
n := node.NewI2pdNode(t, index, ns, ip, port, floodfill)
n.SAMPort = 17656                 // 0 = SAM отключён
n.WriteConfig(zipFile)           // i2pd.conf из i2pd.conf.tmpl
n.Start()                        // i2pd --daemon (форкается сам)
n.CreateReseedZip(dest)          // router.info → ZIP
n.WaitBootstrapped(ctx)          // поллит лог, ищет "NetDbReq: Exploring new"
```

Cleanup: `pkill -f "datadir=<dir> "` + `time.Sleep(2s)`.

Конфиг (`i2pd.conf.tmpl`): `[reseed]` с `{{if .ZipFile}}zipfile = ...{{end}}`,
`[exploratory]` с `inbound.length = 0`, `[ntcp2]`, SSU2 отключён.

**I2P base64:** `base64.RawStdEncoding` + замена `+`→`-`, `/`→`~`.
Не использовать `base64.RawURLEncoding` — даёт `_` вместо `~`, имена файлов в ZIP будут неверными.

## Конвенции

**Ресурсы:** `t.TempDir()` для всех директорий, `t.Cleanup()` для процессов.
Никогда не `defer` на уровне теста — не выполнится при `t.Fatal`.

**Ошибки:** `internal/` возвращает `error`; `testbed/` — `require.NoError`.

**Ожидание:** `context.WithTimeout` + ticker 1s. Никаких `time.Sleep` без
объяснения в комментарии.

**Субпроцессы:** только `os/exec`. `gnunet-arm -s` — `cmd.Start()`.
`i2pd --daemon` — `cmd.CombinedOutput()` (daemon форкается, команда завершается быстро).

## Добавить тест

1. Создать `testbed/<name>_test.go`.
2. Использовать `topology.NewTopology` и типы из `internal/node`.
3. Добавить команду в `Justfile`.

Минимальный пример:

```go
func TestFoo(t *testing.T) {
    tp := topology.NewTopology(t, 2, "10.77.0")

    p := node.NewGnunetPeer(t, 0, tp.Nodes[0].NS, tp.Nodes[0].IP, 2200)
    require.NoError(t, p.WriteConfig())
    require.NoError(t, p.Start())

    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()
    require.NoError(t, p.WaitCoreConnected(ctx))
}
```

## Добавить тип ноды

1. `internal/node/<name>.go` — тип с методами `WriteConfig`, `Start`, `Wait*`.
2. `internal/node/<name>.conf.tmpl` — шаблон конфига.
3. Embed через `//go:embed <name>.conf.tmpl`.

## Rust: sam-sender / sam-receiver

Сейчас — заглушки (`println!` + `exit(0)`).

Сборка: `just build-rs` → `rs/target/debug/sam-sender`, `rs/target/debug/sam-receiver`.

Планируемый интерфейс:
```
sam-sender   --sam <addr:port> --dest <b32> --size <bytes> --seed <n> --out <jsonl>
sam-receiver --sam <addr:port> --out <jsonl>
```

Go-тесты будут вызывать их как субпроцессы через `exec.Command`.

## Docker

Слои образа:
1. `alt:sisyphus`
2. `apt-get install` — gnunet, i2pd, golang, gcc, iputils
3. rustup + PATH
4. `COPY rs/` + `cargo build` (заглушки)

Изменения в `internal/` и `testbed/` не требуют пересборки — они монтируются
через `-v $(pwd):/practice`.

## CI

`.github/workflows/ci.yml` — push в `master`/`main` и PR:
1. Checkout
2. `docker build`
3. `go test ./testbed/... -run Smoke -timeout 5m` внутри `--privileged` контейнера

На GitHub Actions `sudo` не нужен — `docker` доступен напрямую.
