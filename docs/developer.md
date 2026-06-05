# Руководство разработчика

## Архитектура

Проект теперь полностью Rust workspace в корне репозитория.

```
testbed/src/topology.rs
    netns, veth, bridge через std::process::Command

testbed/src/node/
    gnunet.rs      GnunetPeer
    i2pd.rs        I2pdNode, reseed ZIP, RouterInfo hash

sam3/src/
    session.rs     SamClient, StreamSession, StreamListener, ручной SAM3 поверх TCP

sam-bench/src/
    messages.rs    JSON ready/result для transfer-теста
    wire.rs        size + payload + sha256
    payload.rs     детерминированный payload
    sender.rs      sender workflow
    receiver.rs    receiver workflow

sam-sender/src/main.rs
sam-receiver/src/main.rs
    тонкие CLI-обёртки над sam-bench

testbed/tests/
    smoke.rs       test_gnunet_smoke, test_i2pd_smoke
    transfer.rs    test_sam_transfer
```

Всё запускается внутри Docker с `--privileged`.

## Topology

```rust
let tp = Topology::new(4, "10.88.0");
tp.nodes[i].ns;    // "ns_10_88_0<i>"
tp.nodes[i].ip;    // "10.88.0.<i+1>"
tp.nodes[i].veth;  // veth внутри namespace
```

`Drop` удаляет namespace в обратном порядке и затем bridge.

## Node Wrappers

### GnunetPeer

- создаёт tempdir с `.cache/gnunet`, `data/hosts`, `run`;
- рендерит `templates/gnunet.conf.tmpl`;
- стартует `gnunet-arm -s` через `spawn()`;
- экспортирует и импортирует HELLO;
- ждёт CORE-соединение по логу и `gnunet-statistics`;
- `Drop` вызывает `gnunet-arm -e`.

### I2pdNode

- создаёт tempdir и `netDb`;
- рендерит `templates/i2pd.conf.tmpl`;
- стартует `i2pd --daemon` внутри namespace;
- `stop()` использует `pkill -9 -f datadir=<dir>` и проверяет `pgrep`;
- ждёт floodfill RouterInfo по `"Xf"`;
- ждёт bootstrap по `"NetDbReq: Exploring new"` или `"Tunnel: all tunnels built"`;
- создаёт ZIP reseed с I2P base64 RouterInfo hash.

## SAM

`yosemite` был проверен, но не используется: sync API не позволяет точно передать
весь набор tunnel options, нужный для совместимости с предыдущим поведением. Поэтому
`sam3/src/session.rs` реализует SAM3 вручную:

- `HELLO VERSION MIN=3.0 MAX=3.3`
- `DEST GENERATE SIGNATURE_TYPE=7`
- `SESSION CREATE STYLE=STREAM ...`
- `STREAM CONNECT`
- `STREAM ACCEPT`

Публичный STREAM API:

```rust
let client = sam3::SamClient::connect("127.0.0.1:7656");
let session = client.new_stream_session("app", sam3::SAM_TUNNEL_OPTIONS)?;
let dest = session.destination();
let mut outgoing = session.dial(dest)?;
let listener = session.listen();
let mut incoming = listener.accept()?;
```

STREAM session options:

```
inbound.length=0
outbound.length=0
inbound.lengthVariance=0
outbound.lengthVariance=0
inbound.backupQuantity=0
outbound.backupQuantity=0
inbound.quantity=2
outbound.quantity=2
```

## Запуск

```bash
cargo build --release
just test-unit
just test-sam3
just build
just test
just test-gnunet
just test-i2p
just test-transfer
```

`just test-unit` запускает быстрые тесты без Docker: `sam3`, `sam-bench`,
CLI crates и только library-тесты `testbed`. `just test-sam3` проверяет
unit-тесты `sam3` и fake SAM server integration tests.

`just test` и `just test-rust` запускают все Rust-интеграционные тесты
последовательно (`--test-threads=1`), потому что тесты используют одинаковые
имена bridge и namespace.

## Docker

Слои образа:

1. `alt:sisyphus`
2. `apt-get install` — gnunet, libgnunet, i2pd, iproute2, procps, python3, curl, gcc, iputils
3. rustup + PATH
4. `COPY . /workspace`
5. `cargo build --release --manifest-path /workspace/Cargo.toml`
6. копирование `sam-sender` и `sam-receiver` в `/usr/local/bin`

## CI

CI собирает Docker-образ и запускает `just test` внутри privileged контейнера.
