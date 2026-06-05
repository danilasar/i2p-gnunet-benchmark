# AGENTS.md

Инструкции для AI-агентов, работающих с этим репозиторием.

## Контекст

Курсовая работа: методика сравнения транспортных характеристик i2pd/SAM STREAM и GNUnet/CADET.

## Окружение

- **ОС хоста:** ALT Workstation K 11.3 (Nemorosa), ядро 6.12-alt1
- **Контейнер:** Docker, образ `coursework-overlay:latest` на базе `alt:sisyphus`
  - Сборка: `docker build -t coursework-overlay:latest .`
  - Запуск: `docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest <команда>`
- **Версии ПО в контейнере:** i2pd 2.60.0, GNUnet 0.26.2, Rust stable, Python 3.x с networkx

## Правила ведения летописи

- Все действия фиксировать в `letopis.md`, включая ошибки, тупиковые пути и изменения подхода.
- Каждая запись: что сделано, что пошло не так, как решили.
- Обновлять до коммита.

## Правила коммитов

Стиль: `english_tag: русскоязычное описание`

Примеры тегов: `feat`, `fix`, `test`, `build`, `docs`, `refactor`, `chore`.

## Структура проекта

```
.
├── Cargo.toml                # Rust workspace
├── Dockerfile                # alt:sisyphus + gnunet + i2pd + rustup
├── Justfile                  # команды сборки и запуска тестов
├── AGENTS.md
├── letopis.md
├── sam3/                     # SAM3 protocol library
├── sam-bench/                # benchmark helpers over sam3
├── sam-sender/               # CLI sender over sam-bench
├── sam-receiver/             # CLI receiver over sam-bench
├── testbed/
│   ├── src/
│   │   ├── topology.rs       # netns, veth, bridge
│   │   ├── node/             # GnunetPeer, I2pdNode
│   ├── templates/            # i2pd/GNUnet конфиги
│   └── tests/                # smoke и transfer тесты
├── analysis/
└── .github/workflows/ci.yml
```

## Запуск тестов

```bash
just build          # собрать Docker-образ
just test           # все Rust-интеграционные тесты последовательно
just test-gnunet    # только GNUnet smoke
just test-i2p       # только i2pd smoke
just test-transfer  # SAM STREAM transfer
```

`test-rust` сохранён как алиас на `just test`. Все интеграционные тесты требуют
`--privileged`, потому что создают network namespaces и bridge.

## Известные особенности

### GNUnet 0.26.2
- `gnunet-peerinfo` удалён, используются `gnunet-hello` и `gnunet-statistics`.
- Транспорт через communicators (`gnunet-communicator-tcp`), не plugins.
- HELLO-обмен: `gnunet-hello -e` и `gnunet-hello --import`.
- Детекция CORE-соединения: `"notification about connection"` и fallback через statistics.

### i2pd 2.60.0
- `--reseed.urls=` не работает через CLI, reseed задаётся через conf-файл.
- `[reseed] threshold = 0` отключает и ZIP reseed, используется `threshold = 50`.
- Bootstrap: node0 floodfill, остальные через `[reseed] zipfile = ...`.
- RouterInfo hash для ZIP: SHA256(RouterIdentity), I2P base64 (`+`→`-`, `/`→`~`, без padding).
- Для netns с private IP нужны `host`, `nat = false`, `reservedrange = false`.
- Exploratory tunnels должны быть 1-hop; zero-hop exploratory ломает LeaseSet publication.
