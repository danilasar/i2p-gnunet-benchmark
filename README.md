# Testbed: i2pd/SAM STREAM vs GNUnet/CADET

Экспериментальный стенд для сравнения транспортных характеристик
i2pd/SAM STREAM и GNUnet/CADET.

## Текущее состояние

Реализована инфраструктура стенда и smoke-тесты:

| Компонент | Статус |
|-----------|--------|
| Изолированная сеть (netns + veth + bridge) | ✓ |
| GNUnet: запуск пиров, HELLO-обмен, проверка CORE-соединения | ✓ |
| i2pd: запуск нод, ZIP reseed bootstrap, проверка SAM bridge | ✓ |
| Smoke-тесты (`go test`) | ✓ |
| SAM STREAM sender/receiver (Rust) | заглушки |
| CADET sender/receiver | не начато |
| Сбор метрик (goodput, RTT, CPU/RSS) | не начато |
| Underlay-профили (tc/netem) | не начато |
| Анализ результатов (Python) | заглушка |

## Требования

- Docker
- [just](https://github.com/casey/just)

## Запуск

```bash
just build        # собрать Docker-образ (нужен один раз)
just test         # smoke-тесты (~2 мин)
just test-gnunet  # только GNUnet (~15 с)
just test-i2p     # только i2pd (~90 с)
```

Тесты запускаются внутри `--privileged` контейнера (нужны netns и bridge).

## Структура

```
internal/
  topology/        netns, veth, bridge — через t.Cleanup()
  node/
    gnunet.go      GnunetPeer
    i2pd.go        I2pdNode
    *.conf.tmpl    шаблоны конфигов
testbed/
  smoke_test.go    TestGnunetSmoke, TestI2pdSmoke
rs/
  sam-sender/      заглушка
  sam-receiver/    заглушка
analysis/
  analyzer.py      заглушка
.github/
  workflows/
    ci.yml         smoke-тесты при push/PR
```

## ПО в контейнере

| | |
|-|-|
| i2pd | 2.60.0 |
| GNUnet | 0.26.2 |
| Go | из ALT sisyphus |
| Rust | rustup |
| Базовый образ | alt:sisyphus |

## Документация

- [docs/experimenter.md](docs/experimenter.md) — цель стенда, что проверяют smoke-тесты, план измерений
- [docs/developer.md](docs/developer.md) — архитектура, API пакетов, как расширять
