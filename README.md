# Testbed: i2pd/SAM STREAM vs GNUnet/CADET

Экспериментальный стенд для сравнения транспортных характеристик
i2pd/SAM STREAM и GNUnet/CADET.

## Текущее состояние

| Компонент | Статус |
|-----------|--------|
| Изолированная сеть (netns + veth + bridge) | ✓ |
| GNUnet: запуск пиров, HELLO-обмен, проверка CORE-соединения | ✓ |
| i2pd: запуск нод, ZIP reseed bootstrap, проверка SAM bridge | ✓ |
| SAM STREAM sender/receiver на Rust | ✓ |
| Интеграционный transfer-тест 1 MiB через SAM STREAM | ✓ |
| TODO: убрать полный bootstrap в `test_sam_transfer` — сейчас nodes 1-3 заранее получают `RouterInfo` всех нод через `reseed_full.zip`; целевое поведение — discovery от seed/floodfill-ноды без знания всех участников | TODO |
| CADET sender/receiver | не начато |
| Сбор метрик (goodput, RTT, CPU/RSS) | частично |
| Underlay-профили (tc/netem) | не начато |
| Анализ результатов (Python) | заглушка |

## Требования

- Docker
- [just](https://github.com/casey/just)

## Запуск

```bash
just build          # собрать Docker-образ
just test           # все Rust-интеграционные тесты последовательно
just test-gnunet    # только GNUnet smoke
just test-i2p       # только i2pd smoke
just test-transfer  # SAM STREAM transfer
```

Тесты запускаются внутри `--privileged` контейнера, потому что стенд создаёт
Linux network namespaces, veth-пары и bridge.

## Структура

```
Cargo.toml
Cargo.lock
sam3/                библиотека SAM3 + wire/payload + sender/receiver workflows
sam-sender/          Rust CLI для SAM STREAM отправителя
sam-receiver/        Rust CLI для SAM STREAM получателя
testbed/
  src/
    topology.rs      netns, veth, bridge
    node/            i2pd и GNUnet node wrappers
  templates/         i2pd/GNUnet конфиги
  tests/             test_gnunet_smoke, test_i2pd_smoke, test_sam_transfer
analysis/
  analyzer.py        заготовка анализа
```

## Среда стенда

Контейнер основан на **ALT Sisyphus**. Пакеты i2pd и GNUnet ставятся из
репозитория, Rust toolchain ставится через rustup. Проверенные версии:
**i2pd 2.60.0** и **GNUnet 0.26.2**.

## Документация

- [docs/experimenter.md](docs/experimenter.md) — цель стенда, что проверяют тесты, план измерений
- [docs/developer.md](docs/developer.md) — архитектура Rust-кода и расширение стенда
