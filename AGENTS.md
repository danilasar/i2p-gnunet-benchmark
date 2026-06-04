# AGENTS.md

Инструкции для AI-агентов, работающих с этим репозиторием.

## Контекст

Курсовая работа: методика сравнения транспортных характеристик i2pd/SAM STREAM и GNUnet/CADET.

## Окружение

- **ОС хоста:** ALT Workstation K 11.3 (Nemorosa), ядро 6.12-alt1
- **Контейнер:** Docker, образ `coursework-overlay:latest` на базе `alt:sisyphus`
  - Сборка: `sudo docker build -t coursework-overlay:latest .`
  - Запуск: `sudo docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest <команда>`
- **Версии ПО в контейнере:** i2pd 2.60.0, GNUnet 0.26.2, Python 3.x с networkx

## Правила ведения летописи

- Все действия фиксировать в `letopis.md` — **включая ошибки, тупиковые пути и изменения подхода**
- Каждая запись: что сделано, что пошло не так, как решили
- Обновлять до коммита

## Правила коммитов

Стиль: `english_tag: русскоязычное описание`

Примеры тегов: `feat`, `fix`, `test`, `build`, `docs`, `refactor`, `chore`

```
feat: добавлен генератор топологии netns
fix: исправлена детекция bootstrap в GnunetNode
test: smoke-тест i2pd — PASS, 4/4 нод
docs: обновлён AGENTS.md
```

Коммитить при каждом значимом результате, не копить.

## Структура проекта

```
.
├── go.mod / go.sum           # Go-модуль (оркестратор, тесты)
├── Cargo.toml                # Rust workspace (sender/receiver)
├── Dockerfile                # alt:sisyphus + gnunet + i2pd + go + rust
├── Justfile                  # команды сборки и запуска тестов
├── AGENTS.md                 # этот файл
├── letopis.md                # хронология действий
│
├── internal/
│   ├── topology/             # netns, veth, bridge
│   └── node/
│       ├── gnunet.go         # GnunetPeer
│       └── i2pd.go           # I2pdNode
│
├── testbed/
│   └── smoke_test.go         # TestGnunetSmoke, TestI2pdSmoke
│
├── rs/
│   ├── sam-sender/           # заглушка
│   └── sam-receiver/         # заглушка
│
├── analysis/
│   └── analyzer.py           # заглушка
│
└── .github/workflows/ci.yml
```

## Запуск тестов

```bash
# Из корня репозитория
just test                    # все smoke-тесты в контейнере
just test-gnunet             # только GNUnet
just test-i2p                # только i2pd

# Вручную (эквивалент just test)
sudo docker run --rm --privileged -v $(pwd):/workspace -w /workspace \
    coursework-overlay:latest go test ./testbed/... -v -run Smoke -timeout 5m
```

## Известные особенности

### GNUnet 0.26.2
- `gnunet-peerinfo` удалён → используются `gnunet-hello`, `gnunet-statistics`
- Транспорт через communicators (`gnunet-communicator-tcp`), не plugins
- Конфиги: `@INLINE@ /usr/share/gnunet/config.d/*.conf` + секция override
- HELLO-обмен: `gnunet-hello -e` (экспорт), `gnunet-hello --import` (импорт)
- Детекция CORE-соединения: `"notification about connection from"` в логах ARM

### i2pd 2.60.0
- `--reseed.urls=` не работает через CLI → только через conf-файл
- `[reseed] threshold = 0` отключает ВСЁ включая ZIP reseed → использовать `threshold = 50`
- Bootstrap: node0 — floodfill (`--floodfill`), остальные — `[reseed] zipfile = ...`
- Floodfill-бит в RouterInfo: поле `caps` в глобальных опциях (~offset 492), значение `"Xf"`
- `ri_to_netdb.py` вычисляет SHA256(RouterIdentity) → правильный путь в netDb
- Время bootstrap: ~55 секунд до первого DHT exploration
- Детекция связности: `"NetDbReq: Exploring new"` в логах
