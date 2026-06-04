# Testbed: i2pd/SAM STREAM vs GNUnet/CADET

Экспериментальный стенд для курсовой работы «Методика сравнения транспортных
характеристик i2pd/SAM STREAM и GNUnet/CADET».

## Быстрый старт

```bash
# Собрать Docker-образ (нужен один раз)
just build

# Запустить все smoke-тесты
just test
```

Тесты запускаются внутри `--privileged` контейнера — нужны права для создания
Linux network namespaces и bridge-интерфейсов.

## Требования

- Docker
- [just](https://github.com/casey/just) — task runner (`apt-get install just`)
- Права на запуск `sudo docker`

## Структура

```
internal/
  topology/   — создание netns, veth, bridge через t.Cleanup()
  node/
    gnunet.go — GnunetPeer: запуск, HELLO-обмен, ожидание CORE-соединения
    i2pd.go   — I2pdNode: запуск, ZIP reseed bootstrap, ожидание DHT exploration
    *.conf.tmpl
testbed/
  smoke_test.go — TestGnunetSmoke, TestI2pdSmoke
rs/
  sam-sender/   — SAM STREAM sender (Rust, в разработке)
  sam-receiver/ — SAM STREAM receiver (Rust, в разработке)
analysis/
  analyzer.py   — обработка JSONL-логов (Python, в разработке)
```

## Команды

```bash
just build        # собрать Docker-образ
just test         # все smoke-тесты (~2 мин)
just test-gnunet  # только TestGnunetSmoke (~15 сек)
just test-i2p     # только TestI2pdSmoke (~90 сек)
just build-rs     # собрать Rust-бинари
```

## Версии ПО в контейнере

| Компонент | Версия |
|-----------|--------|
| i2pd      | 2.60.0 |
| GNUnet    | 0.26.2 |
| Go        | из ALT sisyphus |
| Rust      | via rustup |

## Топология тестового стенда

Каждый тест поднимает изолированную сеть внутри контейнера:

```
[br_10_88_0]
  ├── veth → ns_10_88_00  (10.88.0.1)  node0 / floodfill
  ├── veth → ns_10_88_01  (10.88.0.2)  node1
  ├── veth → ns_10_88_02  (10.88.0.3)  node2
  └── veth → ns_10_88_03  (10.88.0.4)  node3
```

Bootstrap i2pd: node0 стартует как floodfill, его `router.info` упаковывается
в ZIP-reseed, остальные ноды загружают его при старте.

## Особенности реализации

- `t.Cleanup()` гарантирует удаление netns и остановку процессов даже при падении теста
- `WaitCoreConnected` / `WaitBootstrapped` поллят лог-файл раз в секунду с `context.WithTimeout`
- I2P base64: стандартный base64 с заменой `+`→`-` и `/`→`~` (не URL-safe encoding)
- Reseed ZIP для i2pd требует `threshold ≥ 1` в `[reseed]`; `threshold = 0` отключает всё включая локальный ZIP

## CI

GitHub Actions запускает smoke-тесты при каждом push в `master` и в PR.
См. `.github/workflows/ci.yml`.
