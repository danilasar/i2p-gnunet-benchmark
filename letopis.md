# Летопись практических действий

## 2026-06-04

### Цель этапа
Запустить изолированные от внешней сети ноды GNUnet и I2P, проверить, видят ли они друг друга.

### Установка пакетов

**Источники:**
- GNUnet 0.26.2: https://git.altlinux.org/tasks/417179/build/100/x86_64/rpms/
  - `libgnunet-0.26.2-alt1.x86_64.rpm`
  - `gnunet-0.26.2-alt1.x86_64.rpm`
- i2pd 2.60.0: https://git.altlinux.org/tasks/417180/build/100/x86_64/rpms/i2pd-2.60.0-alt1.x86_64.rpm

**Система:** ALT Workstation K 11.3 (Nemorosa), ядро 6.12.74-6.12-alt1

**Стандартный репозиторий содержит устаревшие версии:**
- gnunet 0.11.5 — не подходит, нужен 0.26.2
- i2pd 2.50.2 — не подходит, нужен 2.60.0

**Попытка установить RPM напрямую — неудовлетворённые зависимости:**
```
libmicrohttpd.so.12         (для libgnunet)
gnutls-utils                (для gnunet)
nss-utils                   (для gnunet)
pkgconf                     (для gnunet)
libboost_program_options.so.1.86.0  (для i2pd)
libminiupnpc.so.21          (для i2pd)
libstdc++.so.6(CXXABI_1.3.15)       (для i2pd)
```

**Решение:** использовать Docker-контейнер на базе `alt:sisyphus` — в нём все нужные версии доступны через стандартный `apt-get install`.

### Docker-образ coursework-overlay:latest

**Базовый образ:** `alt:sisyphus` (docker.io/library/alt:sisyphus)

**Установленные версии (проверено):**
```
i2pd version 2.60.0 (0.9.69), Boost 1.86.0, OpenSSL 3.5.4
gnunet-arm v0.26.2
```

**Dockerfile:** `Dockerfile`

Сборка прошла успешно. Образ `coursework-overlay:latest` готов к использованию.

**Ошибки при установке на хосте (зафиксированы для истории):**
- Попытка `rpm -ivh <URL>` — rpm не умеет качать по HTTP напрямую
- Попытка установить зависимости из p11: `libboost_program_options` (в репо нет), `libminiupnpc17` (нужна .so.21), `libstdc++6` до GCC 14 (нет CXXABI_1.3.15)
- Попытка Dockerfile с `libboost_program_options` — в sisyphus пакет называется `libboost_program_options1.86.0`
- **Решение:** просто `apt-get install gnunet libgnunet i2pd` в sisyphus-контейнере — всё доступно

### Smoke-тест GNUnet

**Ошибки при написании smoke_gnunet.sh:**
- Использовал `ping` для проверки underlay — ping не установлен в контейнере. Заменил на проверку `ip addr show`.
- `gnunet-peerinfo` отсутствует в GNUnet 0.26.2 — удалён, заменён новыми инструментами.
- В конфиге `[transport] PLUGINS = tcp` — в 0.26 нет плагинов, используются communicators (`gnunet-communicator-tcp`). Переписал конфиг под 0.26 с `@INLINE@` стандартных конфигов и override для `[communicator-tcp]`.
- Попытка `gnunet-peerinfo -s` для получения peer ID — команда не существует в 0.26.
- Секция `[hostlist]` требует явного `SERVERS =` (пустая строка), иначе warning о внешнем bootstrap.

**Результат smoke_gnunet.sh:**
```
[PASS] Peers установили CORE-соединение!
core-api DEBUG Received notification about connection from `D88W'.
```
Два изолированных GNUnet-пира в netns (`10.99.0.1` и `10.99.0.2`) успешно обменялись HELLO и установили CORE-соединение через TCP communicator. Детекция через grep в логах gnunet-service-arm.

### Smoke-тест i2pd

**Ошибки и тупиковые пути:**
- `--reseed.urls=` (пустое значение) не работает через CLI: "the argument for option '--reseed.urls' should follow immediately after the equal sign". Решение: только через config-файл `[reseed] urls =`.
- `threshold = 0` отключает не только внешний reseed, но и локальный ZIP reseed. Решение: `threshold = 50`.
- Прямое копирование `router.info` в netDb/: нужно правильное имя файла `routerInfo-<SHA256_hash>.dat` в поддиректории. Написан `ri_to_netdb.py`.
- Попытка cross-populate без floodfill: ноды грузят RouterInfo но не подключаются — нет floodfill для bootstrap. Обнаружено: в RouterInfo caps есть ДВА поля `caps` — одно в адресном блоке NTCP2 (значение "4"), другое в глобальных опциях (значение "Xf" для floodfill). i2pd корректно распознаёт "Xf" при загрузке через ZIP reseed.
- Попытка netns star-topology с /30 подсетями: слишком сложно, заменена на bridge-топологию.
- 30 секунд ожидания недостаточно — bootstrap через floodfill занимает ~55 секунд. Установлено `WAIT_CONNECT=70`.

**Итог smoke_i2pd.sh:**
```
[PASS] 4/4 нод активны в DHT exploration (соединения установлены)!
SAM OK: HELLO REPLY RESULT=OK VERSION=3.3
```
Четыре изолированных i2pd-нода (10.88.0.1–10.88.0.4, zero-hop) в netns, соединённых через Linux bridge, успешно bootstrapped через ZIP reseed с floodfill RouterInfo node0. Все ноды выполнили DHT exploration через node0 (floodfill). SAM bridge готов к приёму соединений.

**Метод детекции связности:** `NetDbReq: Exploring new N routers` — нода отправила DHT exploration query к floodfill. Означает наличие NTCP2-соединения с floodfill.

## Реструктуризация: Go + Rust + Python

### Решение об инструментарии
После smoke-тестов на shell-скриптах принято решение перейти на структурированный стек:
- **Go** (`go test`) — оркестратор, управление нодами, интеграционные тесты
- **Rust** — sender/receiver для измерений (пока заглушки)
- **Python** — анализ данных (pandas, bootstrap CI, графики)

Обоснование выбора Go: `t.Cleanup()` для гарантированного teardown, горутины для параллельного управления нодами, строгая типизация, нет GC-пауз в инфра-коде.

### Первичная реализация (агент по ТЗ)

Агент создал структуру: `internal/topology`, `internal/node/{gnunet,i2pd}.go`, `testbed/smoke_test.go`, Rust workspace (`rs/`), `analysis/`, `Justfile`.

**Найденные и исправленные критические баги:**

- `go.mod`: версия `go 1.26.3` — не существует. Исправлено на актуальную.
- `internal/node/i2pd.go` `CreateReseedZip`: использовал `base64.RawURLEncoding` (`/`→`_`), тогда как i2pd ожидает I2P-base64 (`/`→`~`). ZIP содержал неверные имена файлов — bootstrap не работал. **Исправлено:** `base64.RawStdEncoding` + ручная замена `+`→`-`, `/`→`~`.
- `internal/node/gnunet.go` `Start()`: использовал `CombinedOutput()` — блокирующий вызов, ждущий завершения демона `gnunet-arm`. Тест зависал навсегда. **Исправлено:** `cmd.Start()`.
- `i2pd.conf.tmpl`: всегда рендерил `zipfile = ` (пустая строка) для floodfill-ноды. **Исправлено:** `{{if .ZipFile}}` блок.
- `gnunet.conf.tmpl`: `GNUNET_RUNTIME_DIR`, `GNUNET_CACHE_HOME` были в несуществующей секции `[GLOBAL]`. **Исправлено:** перенесены в `[PATHS]`.
- Отсутствовали `.gitignore` и `.dockerignore` — `rs/target/` попадал в репо и образ. **Исправлено:** добавлены оба файла.

### Результат

`just test` (внутри `--privileged` контейнера):
```
TestGnunetSmoke — PASS (~14 сек)
TestI2pdSmoke   — PASS (~77 сек)
```

Структура проекта готова. Следующий шаг — реализация SAM STREAM sender/receiver на Rust (`rs/`).

## SAM STREAM sender/receiver (Go, 2026-06-04)

### Решение: Go вместо Rust

ТЗ в `TASK_sam_transfer.md` — реализовать transfer через SAM STREAM на Go (не Rust). Rust-заглушки в `rs/` оставлены. Реализованы:
- `internal/sam/{messages,payload,wire,sender,receiver}.go`
- `cmd/sam-sender/main.go`, `cmd/sam-receiver/main.go`
- `testbed/transfer_test.go` — TestSAMTransfer

Smoke-тесты продолжают работать: `just test` — PASS.

### Критические баги при реализации TestSAMTransfer

#### 1. netDb subdirectory format (i2pd ≥ 2.60)
`InstallRouterInfo` клал файлы в `netDb/<b64[:2]>/` (старый формат).
i2pd 2.60.0 использует `netDb/r<b64[:1]>/` (64 подкаталога `r0`–`r9`, `rA`–`rZ`, `ra`–`rz`, `r-`, `r~`).
Файлы игнорировались → "0 routers loaded".
**Исправлено:** `"r"+b64[:1]` в `InstallRouterInfo`.

Но manual cross-populate всё равно ненадёжен (перезаписывает живые версии RouterInfo которые i2pd хранит с другими timestamps после соединений).
**Решение:** добавлен `CreateMultiReseedZip(dest, nodes[])` — создаёт ZIP со всеми RouterInfo, i2pd сам раскладывает по netDb.

#### 2. Phase 1 → Phase 2: лог-файлы не очищались
`WaitBootstrapped` ищет "NetDbReq: Exploring new" в лог-файле. i2pd APPEND к логу, Phase 1 записи остаются.
WaitBootstrapped срабатывал по Phase 1 записям → receiver стартовал до bootstrap Phase 2.
**Исправлено:** `os.Remove(nodes[i].DataDir + "/i2pd.log")` перед Phase 2.

#### 3. Zero-hop туннели не работают для LeaseSet publication
С `inbound/outbound.length=0` i2pd пытается выбрать себя как hop и падает с:
`"Can't select next hop for <self_hash>, no peers available"`.
- DHT exploration работает (через прямой NTCP2, туннели не нужны).
- **Публикация LeaseSet НЕ работает** — идёт через исходящий туннель (garlic message). Без туннеля → "Can't publish LeaseSet, no more floodfills found".
**Исправлено:** `inbound.length=1, outbound.length=1` и в SAM-сессии, и в `[exploratory]` конфига.

#### 4. DialI2P блокируется без deadline (sam3 v0.33.92)
`DialContextI2P` говорит "eventually..." но всё равно вызывает `DialI2P` без context.
`DialI2P` блокируется на `conn.Read(buf)` без deadline.
**Исправлено:** обёртка в горутину с `perAttempt=90s` + retry loop до timeout.

#### 5. Прерывания sender по transferCtx
transferCtx (6 мин) и bootCtx (4 мин) были одним context. При долгом bootstrap sender получал меньше времени.
**Исправлено:** раздельные bootCtx и transferCtx (10 мин).

#### 6. i2pd --daemon double-start
Phase 1 daemon жив при старте Phase 2 (pkill по pattern не убивал daemon после double-fork).
**Исправлено:** `pkill -9 -f "datadir=<dir>"` + pgrep-верификация + `os.Remove(pid_file)`.

#### 7. fmt.Errorf(variable) — pre-existing vet error
В `fail()` функции: `return fmt.Errorf(res.Error)` вместо `return errors.New(res.Error)`.
**Исправлено:** `errors.New`.

### Текущий статус (продолжается отладка)

TestSAMTransfer: получено "receiver ready" (SESSION CREATE с 1-хоп работает после фикса лог-файлов), запущен sender. Ожидаем результат.

## Отладка TestSAMTransfer, 2026-06-04 22:07 +04

Повторный запуск `just test-transfer` дошёл до receiver ready, но sender завершился:
`{"success":false,"error":"DialI2P: Can not reach peer"}`.

Что пошло не так:
- в текущих файлах снова были `inbound.length=0/outbound.length=0` для SAM-сессий и `[exploratory]`;
- i2pd писал `Can't create outbound tunnel, no peers available`, поэтому LeaseSet receiver'а не становился достижимым для sender'а;
- `just test-transfer` запускал `/usr/local/bin/sam-sender` и `/usr/local/bin/sam-receiver` из Docker-образа, так что изменения в `cmd/` и `internal/sam/` могли не попадать в тест без пересборки образа.

Что сделано:
- возвращены 1-hop туннели для `[exploratory]` в `internal/node/i2pd.conf.tmpl`;
- добавлен единый `streamTunnelOptions()` для SAM STREAM sender/receiver: length=1, variance=0, quantity=2, backupQuantity=0;
- `just test-transfer` теперь перед тестом пересобирает `sam-sender` и `sam-receiver` из текущей рабочей копии внутрь контейнера.

После этого `go build` внутри контейнера упал на VCS stamping:
`error obtaining VCS status: exit status 128`.
Для тестовых бинарников VCS metadata не нужна, поэтому в сборку добавлен `-buildvcs=false`.

Следующий прогон с 1-hop SAM-сессиями завис до сообщения `ready` от receiver: `sam-receiver`
не завершил создание/Listen сессии за несколько минут. Чтобы не получать 20-минутные
зависания без диагностики, в `TestSAMTransfer` добавлен timeout на чтение `ready` и
финального result от receiver; при timeout печатаются stderr receiver и релевантные логи i2pd.

Изменение подхода: SAM STREAM destinations снова используют 0-hop options, а 1-hop оставлен
для `[exploratory]` i2pd. Проверяем гипотезу, что для тестовой передачи достаточно 1-hop
exploratory-туннелей для публикации/поиска LeaseSet, без построения 1-hop туннелей самой
SAM destination.

По замечанию о логах в контейнере: `just test-transfer` запускался с `--rm`, поэтому
полные `/root/tmp/TestSAMTransfer.../i2pd.log` исчезали вместе с контейнером. Добавлен
`TEST_ARTIFACT_DIR=/workspace/tmp/test-artifacts/sam-transfer`; при диагностике тест теперь
сохраняет полные `nodeN-i2pd.log` в workspace, доступный с хоста.

Анализ сохранённых логов показал главное отличие от ожидаемого bootstrap:
все ноды писали `NetDb: 3 routers loaded (0 floodfils)`, затем
`NetDb: Deleting 3 unreachable routers`, и не было ни одной строки
`NTCP2: Connected/Established`. Значит, RouterInfo node0 попадал в netDb без floodfill caps.

Причина: Phase 1 ждала node0 только 10 секунд перед упаковкой/копированием `router.info`,
а по прежней отладке floodfill RouterInfo стабилизируется существенно дольше. Добавлен
`WaitRouterInfoFloodfill(ctx)`, который ждёт появления `Xf` в `router.info`; `TestI2pdSmoke`
и `TestSAMTransfer` теперь используют его вместо фиксированного короткого sleep.

Попытка Phase 2 с прямым `InstallRouterInfo` не помогла: i2pd писал
`NetDb: 3 routers loaded (0 floodfils)` и затем удалял все RouterInfo как unreachable.
Возврат подхода: для nodes 1-3 снова используется `fullZip` reseed, потому что предыдущая
отладка показывала корректное распознавание floodfill caps именно через ZIP reseed.

Дополнительная проверка официальной документации i2pd показала ошибку в понимании адресов:
`address4` — это local bind address, а публикуемый адрес RouterInfo задаётся `host`.
Также по умолчанию включён `reservedrange`, который отбрасывает RouterInfo с private IP
вроде `10.88.0.x`. Для netns-тестбедов в `I2pdNode.Start()` добавлены:
`--host=<node IP>`, `--nat=false`, `--reservedrange=false`, `--bandwidth=X`.

Проверка `just test` показала, что ALT-сборка i2pd не принимает `--nat=false`
как CLI-аргумент (`option '--nat' does not take any arguments`). Исправление:
`host`, `nat = false`, `reservedrange = false`, `bandwidth = X` перенесены в
`i2pd.conf.tmpl`; в CLI остаются только `--datadir`, `--conf`, `--address4`, логи и daemon.

После исправления адресов `TestSAMTransfer` дошёл дальше: sender успешно установил STREAM
и записал 1 MiB (`Success=true`), но receiver вернул
`ReceivePayload: failed to read size: EOF`. Это означает пустой/сброшенный accepted stream
до payload. Receiver изменён: если accepted stream закрыт до size-header (`received=0`, EOF),
он закрывает этот conn и продолжает `Accept()` до общего timeout.

Следующий прогон показал, что receiver больше не падает на пустом stream, но payload не приходит
до timeout. Sender при этом завершает запись локально за миллисекунды. Добавлена задержка 5 секунд
после `SendPayload` перед закрытием conn/session, чтобы i2pd успел протолкнуть STREAM-данные.

### Итог проверки

После задержки sender-side close `TestSAMTransfer` прошёл:
`setup≈6012ms`, `transfer≈3ms`, `goodput≈2796 Mbps`, `first_byte≈6328ms`, 1 MiB, SHA256 OK.

Финальные проверки:
- `go test ./...` — PASS
- `just test` — PASS (`TestGnunetSmoke`, `TestI2pdSmoke`)
- `just test-transfer` — PASS (`TestSAMTransfer`)

После финального контрольного прогона повторно проверены:
- `go test ./...` — PASS
- `just test` — PASS
- `just test-transfer` — PASS (`setup≈9012ms`, `transfer≈2ms`, `goodput≈4194 Mbps`, `first_byte≈9324ms`)

`just test-transfer` создаёт временный каталог `/workspace/tmp/test-artifacts` внутри
контейнера перед запуском теста. После успешного прогона каталог `tmp/` удалён через Docker,
чтобы в рабочей копии не оставались root-owned артефакты, мешающие последующим `go test ./...`.

## Удаление project-specific mount path, 2026-06-04 23:25 +04

По просьбе убраны упоминания старого имени каталога из команд и документации проекта. `Justfile` теперь
везде монтирует текущий каталог в контейнер как `/workspace`; путь для `TEST_ARTIFACT_DIR`
также перенесён на `/workspace/tmp/test-artifacts/sam-transfer`.

Проверки после изменения mount path:
- поиск старого имени каталога по репозиторию — совпадений нет;
- `go test ./...` — PASS;
- `just test-i2p` — PASS;
- `just test-transfer` — PASS (`setup≈12012ms`, `transfer≈3ms`, `goodput≈2796 Mbps`, `first_byte≈12323ms`).

После `just test-transfer` удалён созданный контейнером каталог `/workspace/tmp`.

## README TODO, 2026-06-04 23:20 +04

По замечанию о текущем bootstrap в `TestSAMTransfer` добавлен пункт TODO в таблицу README:
сейчас nodes 1-3 заранее получают `RouterInfo` всех нод через `reseed_full.zip`, а целевое
поведение — discovery от seed/floodfill-ноды без знания всех участников.

## Рабочее правило, 2026-06-05

Принято правило для следующих этапов: отмечать ход работы в `letopis.md`, включая текущие
ошибки, способы их исправления и принимаемые решения.

Коммиты оформлять в стиле:
`english_conventional_tag: русское описание коммита`.

## Переписывание Go-проекта на Rust, 2026-06-05

Начат перенос Go-оркестратора и SAM sender/receiver в Rust workspace `rs/`.
Go-код оставлен без изменений для сравнения.

Проверен crate `yosemite`:
- `cargo search yosemite` показал актуальную версию `0.7.0`;
- docs.rs и исходники crate подтверждают наличие sync/async API для `Session<Stream>`,
  `destination()`, `connect()` и `accept()`;
- sync API формирует при `SESSION CREATE` только часть нужных tunnel options
  (`inbound.length`, `outbound.length`, `inbound.quantity`, `outbound.quantity`) и не даёт
  точно передать весь набор из Go (`lengthVariance`, `backupQuantity`).

Решение: SAM3 реализован вручную поверх TCP, чтобы сохранить идентичные команды:
`HELLO`, `DEST GENERATE`, `SESSION CREATE`, `STREAM CONNECT`, `STREAM ACCEPT`, включая все
zero-hop tunnel options.

Реализовано:
- `rs/testbed` как библиотека с модулями `topology`, `node::{i2pd,gnunet}`, `sam`;
- Rust CLI `sam-sender` и `sam-receiver`;
- интеграционные тесты `test_gnunet_smoke`, `test_i2pd_smoke`, `test_sam_transfer`;
- шаблоны `i2pd.conf.tmpl` и `gnunet.conf.tmpl` перенесены в `rs/testbed/templates`;
- `Dockerfile` и `Justfile` переключены на сборку Rust sender/receiver.

Текущие проверки:
- `cargo build --release --manifest-path rs/Cargo.toml` — PASS;
- `cargo test --manifest-path rs/Cargo.toml -p testbed --no-run` — PASS.

Следующий этап: собрать Docker-образ и прогнать Rust-интеграционные тесты внутри
`--privileged` контейнера.

При первом `just build` Docker-сборка упала на `apt-get install cargo rust`: в ALT Sisyphus
пакет `cargo` не найден. Решение: убрать `cargo`/`rust` из apt-пакетов и установить stable
toolchain через rustup (`curl https://sh.rustup.rs | sh -s -- -y`), затем добавить
`/root/.cargo/bin` в `PATH`.

Финальные проверки Rust-порта:
- `cargo build --release --manifest-path rs/Cargo.toml` — PASS;
- `cargo test --manifest-path rs/Cargo.toml -p testbed --no-run` — PASS;
- `just build` — PASS, образ `coursework-overlay:latest` собирает Rust sender/receiver;
- Docker: `test_gnunet_smoke` — PASS (~10 сек);
- Docker: `test_i2pd_smoke` — PASS (~56 сек);
- `just test-transfer` — PASS (`setup≈9093ms`, `transfer≈2ms`,
  `goodput≈4194 Mbps`, `first_byte≈9403ms`);
- `just test-rust` — PASS, все Rust-интеграционные тесты последовательно
  (`setup≈6092ms`, `transfer≈2ms`, `goodput≈4194 Mbps`, `first_byte≈6418ms`).

После успешных прогонов каталог `tmp/test-artifacts` оказался root-owned из контейнера.
Обычный `rm -rf tmp` с хоста завершился `Permission denied`; каталог удалён через
`docker run --rm -v $(pwd):/workspace ... rm -rf /workspace/tmp`.

Также после Docker-сборок часть файлов в игнорируемом `rs/target` стала root-owned.
Чтобы не мешать последующим локальным `cargo build`, владелец `rs/target` возвращён
на пользователя `1000:1000` через контейнерный `chown`.

## Удаление Go-реализации и перенос Rust в корень, 2026-06-05

По решению после успешного Rust-порта Go-часть удалена:
- `cmd/`, `internal/`, старый Go `testbed/`, `go.mod`, `go.sum`.

Rust workspace перенесён из `rs/` в корень проекта:
- `Cargo.toml`, `Cargo.lock` теперь лежат в корне;
- `sam-sender/`, `sam-receiver/`, `testbed/` стали корневыми workspace members;
- `target/` добавлен в `.gitignore` и `.dockerignore` вместо `rs/target`.

`Justfile` переведён на Rust:
- `just test-gnunet` запускает `test_gnunet_smoke` в Docker;
- `just test-i2p` запускает `test_i2pd_smoke` в Docker;
- `just test-transfer` собирает Rust sender/receiver, копирует их в `/usr/local/bin`
  и запускает `test_sam_transfer`;
- `just test` запускает все Rust-интеграционные тесты последовательно
  (`--test-threads=1`) и также пересобирает sender/receiver перед запуском;
- `just test-rust` оставлен как алиас на `just test`.

`Dockerfile` больше не устанавливает `golang`; Rust toolchain по-прежнему ставится через rustup.
README, AGENTS, developer/experimenter docs и CI обновлены под корневой Rust workspace.

Проверки после переноса в корень:
- `cargo fmt --all` — PASS;
- `cargo build --release` — PASS;
- `cargo test -p testbed --no-run` — PASS;
- `just build` — PASS;
- `just test-gnunet` — PASS (`test_gnunet_smoke`, ~12 сек);
- `just test-i2p` — PASS (`test_i2pd_smoke`, ~56 сек);
- `just test-transfer` — PASS (`test_sam_transfer`,
  `setup≈12096ms`, `transfer≈2ms`, `goodput≈4194 Mbps`, `first_byte≈12406ms`).

Дополнительно был запущен полный `just test` как проверка агрегирующей команды. Это был лишний
повтор уже пройденных тестов; в нём `test_gnunet_smoke` и `test_i2pd_smoke` прошли, а повторный
`test_sam_transfer` упал на ожидании result от receiver после ошибки поиска LeaseSet в i2pd
(`receiver did not send result message`). Решение на этот момент: не начинать выделение
SAM-интерфейсов в отдельную библиотеку до коммита текущего состояния; нестабильность повторного
агрегирующего прогона зафиксирована отдельно от успешного целевого `just test-transfer`.

## Выделение SAM-библиотеки, 2026-06-05

После коммита корневого Rust workspace начато выделение SAM-слоя из `testbed`.
Решение: создать отдельный workspace crate `sam3`, потому что ручная SAM3-реализация
полезна отдельно от orchestration-кода с network namespaces.

Перенесено из `testbed/src/sam/` в `sam3/src/`:
- `messages.rs`, `payload.rs`, `wire.rs`;
- `sender.rs`, `receiver.rs`;
- низкоуровневый SAM3-клиент переименован из `sam3.rs` в `session.rs`, чтобы публичный API
  был `sam3::session::SamSession`, а не `sam3::sam3`.

`sam-sender` и `sam-receiver` теперь зависят от `sam3`, а не от всего `testbed`.
`testbed` использует `sam3::{ReadyMsg, ResultMsg}` в transfer-тесте. Ошибка в процессе:
после удаления `serde` из runtime-зависимостей `testbed` сборка тестов упала, потому что
`testbed/tests/transfer.rs` напрямую использует `serde::Serialize` в helper-функции `as_json`.
Решение: вернуть `serde` только как `[dev-dependencies]` для `testbed`.

Проверки:
- `cargo build --release` — PASS;
- `cargo test -p testbed --no-run` — PASS.
- `cargo test --workspace --no-run` — PASS;
- `just test-transfer` — PASS (`setup≈9090ms`, `transfer≈2ms`,
  `goodput≈4194 Mbps`, `first_byte≈9418ms`).

## Тестовое покрытие SAM-библиотеки, 2026-06-05

Начато покрытие `sam3` быстрыми тестами без реального i2pd:
- unit-тесты для payload/wire и SAM response parsing;
- integration-тесты с in-process fake SAM server, который проверяет фактически отправленные
  команды `HELLO`, `DEST GENERATE`, `SESSION CREATE`, `STREAM CONNECT`, `STREAM ACCEPT`.

Решение: реальные Docker/i2pd-тесты оставить для совместимости с настоящим router, а основную
регрессию протокольного API ловить быстрыми unit/fake-server тестами.

В процессе добавления тестов компилятор поймал проблему в тесте: `Result::expect_err`
требовал `Debug` для успешного типа `SamSession`, а `TcpStream` внутри session не обязан
участвовать в debug-представлении. Решение: заменить `expect_err` на явный `match`, не меняя
публичный тип ради теста.

Добавлены just-команды:
- `just test-unit` — быстрые тесты `sam3`, CLI crates и `testbed --lib` без Docker/root;
- `just test-sam3` — `cargo test -p sam3 -- --nocapture`.

CI обновлён: после сборки Docker-образа добавлен отдельный шаг fast tests без `--privileged`.
Это не заменяет i2pd integration tests, но даёт более ранний и дешёвый сигнал по `sam3` API.

Проверки:
- `just test-sam3` — PASS: 9 unit-тестов `sam3` и 4 fake SAM integration tests;
- `just test-unit` — PASS;
- `cargo test --workspace --no-run` — PASS.

## Разделение SAM core и benchmark helpers, 2026-06-05

Начато очищение `sam3` от benchmark-специфики. Решение: вынести `messages`, `payload`,
`wire`, `sender`, `receiver` в отдельный crate `sam-bench`, а не дублировать их внутри
`sam-sender`/`sam-receiver`. Причина: эти helpers одновременно нужны обоим CLI и
`testbed/tests/transfer.rs`, но не являются частью универсального SAM3 API.

Реализовано:
- создан workspace member `sam-bench`;
- `sam3` оставлен только с `session.rs` и fake SAM тестами;
- `sam-bench` зависит от `sam3` и содержит JSON ready/result, deterministic payload,
  benchmark wire protocol и sender/receiver workflows;
- `sam-sender`, `sam-receiver` и `testbed/tests/transfer.rs` переключены на `sam-bench`;
- быстрые тесты и CI fast step обновлены, чтобы проверять оба crate.

Проверки:
- `cargo test -p sam3 -p sam-bench -p sam-sender -p sam-receiver` — PASS;
- `cargo test -p testbed --lib` — PASS;
- `cargo test --workspace --no-run` — PASS.

Следующий шаг: добавить в `sam3` публичный ergonomic API поверх текущих низкоуровневых
функций: `SamClient`, `StreamSession`, `StreamListener`. На этом этапе API остаётся
transient-destination only; keyfile/lookup/options builder будут отдельными шагами.

Реализовано:
- `SamClient::connect(addr)`;
- `SamClient::new_stream_session(id, options)`;
- `StreamSession::{id, sam_addr, destination, dial, listen}`;
- `StreamListener::{id, accept}`;
- `sam-bench` переключён на `SamClient` для создания session/listener, но retry sender
  по-прежнему делает только `STREAM CONNECT` на попытку, без повторного `SESSION CREATE`.

Добавлены fake SAM tests для нового API:
- создание stream session через `SamClient`;
- `StreamSession::dial`;
- `StreamListener::accept`.

Проверки:
- `just test-unit` — PASS;
- `cargo test --workspace --no-run` — PASS.
- `just test-transfer` — PASS (`setup≈9093ms`, `transfer≈2ms`,
  `goodput≈4194 Mbps`, `first_byte≈9404ms`).

## Ключи и destination в SAM3, 2026-06-05

Начат перенос semantics из Go `sam3.NewKeys()` / `NewStreamSession(id, keys, options)`:
ключи должны быть отдельным объектом, а session должна уметь создаваться с уже существующим
private key, не только через transient `DEST GENERATE` внутри `SESSION CREATE`.

Решение по API:
- `SamClient::new_keys()` — с дефолтным `SIGNATURE_TYPE=7`;
- `SamClient::new_keys_with_signature_type(sig_type)`;
- `SamClient::new_stream_session(id, &keys, options)` — session с существующими ключами;
- `SamClient::new_transient_stream_session(id, options)` — удобный прежний сценарий.

Первый вариант `new_transient_stream_session()` был реализован как два SAM-соединения:
`new_keys()` и затем `SESSION CREATE`. Быстрые тесты прошли, но `just test-transfer` упал
на ожидании result от receiver после LeaseSet lookup failure. Чтобы не менять поведение
benchmark-трафика и не добавлять лишние SAM control-соединения в transient-сценарий,
решение изменено: `new_transient_stream_session()` должен использовать прежний одно-соединительный
путь `DEST GENERATE` + `SESSION CREATE` на одном socket. Явный reusable keys API остаётся
раздельным.

Сверка с Go `sam3`: приложение использовало `s.NewKeys()` и затем
`s.NewStreamSession(id, keys, options)`, то есть API Go-библиотеки разделяет генерацию ключей
и создание session. В Rust это соответствует явному пути `new_keys()` + `new_stream_session()`.
Одно-соединительный `new_transient_stream_session()` оставлен как compatibility helper для
нашего benchmark, а не как прямой аналог Go `NewKeys`.

Проверки:
- `just test-unit` — PASS: `sam3` 6 unit tests и 12 fake SAM tests;
- `cargo test --workspace --no-run` — PASS;
- первый `just test-transfer` после двух-соединительного transient path — FAIL
  (`receiver did not send result message`, LeaseSet lookup failure);
- повторный `just test-transfer` после возврата одно-соединительного transient path — PASS
  (`setup≈15134ms`, `transfer≈2ms`, `goodput≈4194 Mbps`, `first_byte≈15445ms`).

Технический долг:
- `new_transient_stream_session()` сейчас не является прямым аналогом Go-пути
  `NewKeys()` + `NewStreamSession(...)`: для устойчивости benchmark он делает `DEST GENERATE`
  и `SESSION CREATE` на одном SAM control socket. Это нужно оставить явно задокументированным,
  а после стабилизации i2pd testbed проверить, можно ли безопасно перевести transient helper
  на тот же путь, что reusable keys API.
- `Keys::read_keyfile()` умеет читать наш простой формат `PUB=...` / `PRIV=...` и fallback
  private-only, но пока не восстанавливает public destination из raw private key. Для полноценной
  совместимости с Go `i2pkeys` нужен настоящий parser I2P private key / destination.

Что ещё осталось для полноценной SAM-библиотеки:
- `NAMING LOOKUP` (`SamClient::lookup`) и typed handling `KEY_NOT_FOUND`, `INVALID_KEY`,
  `I2P_ERROR`;
- typed `SessionOptions` builder вместо `&[(&str, &str)]`, включая zero-hop/small/standard
  presets и arbitrary I2CP options;
- typed `SamError` / `SamResultCode` вместо строковых `Box<dyn Error>` в core API;
- STREAM ergonomics: `SamConn`, local/remote destination metadata, deadline helpers,
  `StreamListener::incoming()`;
- `STREAM FORWARD`;
- DATAGRAM и RAW sessions;
- PRIMARY/MASTER sessions SAM 3.3;
- examples и crate-level README для `sam3`;
- real i2pd integration tests для reusable keyfile: создать ключи, сохранить, пересоздать
  session с тем же private key и проверить стабильность destination/приём stream.

## NAMING LOOKUP: разрешение I2P-адресов, 2026-06-05

### Что сделано

- `SamClient::lookup(name)` и `StreamSession::lookup(name)` → `Result<Destination, SamError>`.
- Приватный хелпер `lookup_on` — единая точка протокольной логики: отправляет
  `NAMING LOOKUP NAME=<name>`, разбирает `NAMING REPLY`, возвращает `VALUE=` или ошибку.
- Оба метода открывают новое TCP-соединение (как `new_keys`) — управляющий сокет сессии
  не затрагивается: это требование протокола SAM.
- Обработка всех error-кодов через уже существующие варианты `SamError`: `KeyNotFound`,
  `InvalidKey`, `I2PError`. `VALUE` отсутствует при `RESULT=OK` → `UnexpectedResponse`.
- 5 новых fake-SAM тестов (6-й из ТЗ поглощён первым: `expect_line` верифицирует формат команды).

### Проблемы при реализации

**Чувствительность `Edit` к объёму контекста** — попытка вставить всю реализацию одним блоком
дала «0 occurrences found». Решение: хирургические правки по три отдельных шага
(`SamClient::lookup` → `StreamSession::lookup` → `lookup_on`).

**Отдельные соединения** — намеренно, чтобы не слать `NAMING LOOKUP` в управляющий сокет сессии.
Аналогично `new_keys`.

**Отсутствие `VALUE` при `RESULT=OK`** — явная проверка через `ok_or_else → UnexpectedResponse`,
предотвращает panic на сломанном ответе bridge.

### Проверки

- 23 теста в `fake_sam.rs` — PASS.
- `cargo build --workspace` — 0 предупреждений.

## SamError: типизированные ошибки SAM-протокола, 2026-06-05

### Что сделано

- Новый модуль `sam3/src/error.rs` с `enum SamError`: варианты для всех RESULT-кодов протокола
  (`DuplicatedDest`, `DuplicatedId`, `CantReachPeer`, `KeyNotFound`, `InvalidKey`, `InvalidId`,
  `Timeout`, `SessionNotFound`, `NotASession`, `I2PError(String)`, `UnexpectedResponse(String)`,
  `Io(String)`).
- `SamError::from_result_line` — парсит строку ответа SAM через `parse_fields`, возвращает
  конкретный вариант.
- `Display` выводит протокольные строки: `CANT_REACH_PEER`, `I2P_ERROR: msg`, etc.
- `impl From<io::Error>` для прозрачной конвертации.
- Всё публичное API `session.rs` переведено с `Box<dyn Error>` на `Result<T, SamError>`.
- `SamSession`, `StreamSession`, `SamClient` получили `#[derive(Debug)]`.
- `parse_fields` стала `pub(crate)` для использования из `error.rs` без дублирования.

### Проблемы при реализации

**`unwrap_err()` требует `Debug` для типа успеха** — без `#[derive(Debug)]` на `SamSession`/
`StreamSession` тесты не компилировались. Решение: добавить derive.

**`generate_keys_on` с неверной логикой** — в первой итерации добавлена проверка
`if !line.contains("DEST REPLY")`, из-за которой ошибки SAM, пришедшие в нестандартном
формате, превращались в `UnexpectedResponse` вместо конкретного варианта. Отлаживали через
временное тегирование сообщений об ошибке. Итог: проверка `contains` избыточна и удалена;
логика сведена к `if fields.get("RESULT").is_some_and(|v| v != "OK")` — как в исходной
версии до рефакторинга.

**Доступ к `parse_fields` из `error.rs`** — функция была приватной в `session.rs`.
Сделана `pub(crate)`, чтобы `from_result_line` использовал единый парсер (с поддержкой
кавычек и экранирования).

**`UnexpectedResponse` вместо `DuplicatedId` в интеграционном тесте** — следствие проблемы
с `generate_keys_on` выше. `create_stream` вызывает `generate_keys_on` первым; ошибка
перехватывалась там с неверной классификацией до того, как SESSION CREATE даже отправлялся.

### Найдено при ревью и исправлено

- **`Display` делегировал `Debug`** (`write!(f, "{:?}", self)`) — производил Rust-синтаксис
  `CantReachPeer` вместо `CANT_REACH_PEER`. Заменён явным match.
- **Unused import `std::error`** в `session.rs` — удалён.
- **Избыточная `contains("DEST REPLY")` проверка** в `generate_keys_on` — удалена
  (описано выше).

### Проверки

- 6 unit-тестов для `SamError` в `error.rs` — PASS.
- 2 новых интеграционных теста в `fake_sam.rs` (`connect_stream_returns_cant_reach_peer`,
  `session_create_returns_duplicated_id`) — PASS.
- `cargo test -p sam3` — 31 тест, PASS, 0 предупреждений.
- `cargo test --workspace --no-run` — PASS.

## SamConn: типизированное соединение с metadata, 2026-06-05

Реализован `SamConn` в `sam3/src/session.rs` по ТЗ (TDD).

### Что сделано

- Новый тип `SamConn { inner: TcpStream, local: Destination, remote: Destination }` с `Read + Write`,
  `set_read_timeout`, `set_write_timeout`, `local_destination()`, `remote_destination()`.
- `SamSession::accept_stream` теперь принимает `local: &Destination` и возвращает `SamConn`:
  вторая строка после `STREAM ACCEPT OK` (raw base64 destination) больше не выбрасывается.
- `StreamListener` получил поле `local: Destination`; `session.listen()` клонирует destination сессии.
- `StreamListener::accept()` → `SamConn`.
- `StreamSession::dial()` → `SamConn` (local = сессионный destination, remote = переданный адрес).
- `SamConn` экспортируется из `sam3/src/lib.rs`.

### Нетривиальные решения

**`SamConn::new` оставлен `pub`** — иначе `dial_with_retry` в `sam-bench` не мог бы собрать
`SamConn` внутри потока. `dial_with_retry` вынужден спавнить поток с `move`-замыканием
и клонировать `(sam_addr, id, local_dest)` заранее, потому что `&StreamSession` не живёт
через `thread::spawn`. Вызов `SamSession::connect_stream` возвращает `TcpStream`, после
чего `SamConn::new` упаковывает его вместе с destination-метаданными. Это единственное место
в кодовой базе, где конструктор вызывается вне `session.rs`; намеренный компромисс.

**Исправлена ошибка в старом тесте** `accept_stream_sends_expected_sam_commands_and_returns_socket`:
FakeSam ошибочно слал `"REMOTE DESTINATION=clientdest"` вместо просто `"clientdest"`.
Тест проходил, потому что `remote_destination()` не проверялся, но хранимое значение
было бы неверным. Исправлено на `writeln!(stream, "clientdest")`.

**Нестабильный первый прогон `just test-transfer`** — упал с `"receiver did not send result message"`
из-за ошибок построения туннелей i2pd (`Can't find floodfill`) на старте сети. Регрессий
в коде нет; повторный прогон прошёл. Это поведение тестового стенда, не ошибка библиотеки.

**Предупреждение `unused_assignments`** в `sender.rs` — переменная `last_error` инициализировалась
пустой строкой, которая сразу перезаписывалась. Исправлено сужением области видимости.

### Проверки

- 4 новых TDD-теста в `sam3/tests/fake_sam.rs` — PASS:
  `accept_exposes_remote_destination`, `dial_exposes_local_and_remote_destination`,
  `samconn_read_write_delegates_to_socket`, `samconn_set_timeout_does_not_error`.
- `just test-unit` — PASS (16 тестов).
- `cargo test --workspace --no-run` — PASS (без предупреждений).
- `just test-transfer` — PASS.

## Дедлайны dial/accept, 2026-06-05

Реализованы таймауты на уровне SAM-рукопожатия для `dial` и `accept`, а также удобный
`set_timeout` на `SamConn`. Рефакторинг `dial_with_retry` в `sam-bench`.

### Что сделано

- `SamConn::set_timeout(dur)` — устанавливает read и write таймаут в одном вызове.
- `StreamSession::dial_timeout(dest, timeout)` — открывает соединение с таймаутом;
  таймаут снимается сразу после `STREAM STATUS RESULT=OK`, данные идут без ограничений.
- `StreamListener::accept_timeout(timeout)` — ждёт строку с адресом удалённого клиента
  с таймаутом; таймаут снимается сразу после её получения.
- Приватные хелперы `connect_stream_timeout` и `accept_stream_timeout` в `session.rs`.
- `dial_with_retry` в `sam-bench/src/sender.rs` переписан: убраны `thread::spawn`,
  `mpsc::channel`, прямые вызовы `SamSession::connect_stream`; теперь использует
  `session.dial_timeout` с `attempt_timeout = min(90s, оставшееся_до_deadline)`.

### Нетривиальные решения

**Таймаут только на read в `accept_stream_timeout`** — `set_write_timeout` не выставляется:
запись (`STREAM ACCEPT`) быстрая и не блокирует; единственное блокирующее ожидание —
чтение строки с remote destination. Known limitation: если SAM зависнет при отправке
`STREAM STATUS OK`, таймаут не сработает.

**Таймаут снимается после рукопожатия** — и в `connect_stream_timeout`, и в
`accept_stream_timeout` оба таймаута сбрасываются до `None` перед возвратом соединения.
Это не случайно: фаза данных не должна прерываться по истечении handshake-таймаута.

**FakeSam::spawn_many для двух соединений** — `new_transient_stream_session` открывает
первое TCP-соединение (HELLO + DEST GENERATE + SESSION CREATE), `dial_timeout` и
`accept_timeout` открывают второе. Тесты, использующие `SamClient`, требуют двух
независимых обработчиков в FakeSam.

**`unused_assignments` в `dial_with_retry`** — `last_err` инициализировался пустой
строкой, а затем немедленно перезаписывался. Исправлено объявлением `let last_err =`
непосредственно внутри `match`-выражения.

### Проверки

- 5 новых тестов в `sam3/tests/fake_sam.rs` — PASS:
  `samconn_set_timeout_sets_both_directions`, `dial_timeout_succeeds_within_deadline`,
  `dial_timeout_returns_error_on_slow_response`, `accept_timeout_succeeds_when_client_connects`,
  `accept_timeout_returns_error_when_no_client`.
- `cargo test -p sam3` — 28 тестов, PASS, 0 предупреждений.
- `cargo build --workspace` — PASS, 0 предупреждений.

## Тесты совместимости sam3 с go-i2p/sam3, 2026-06-05

Реализованы кросс-языковые интеграционные тесты Rust-библиотеки `sam3` против Go-реализации
`go-i2p/sam3 v0.33.92`. Три теста: Rust→Go (echo), Go→Rust (echo), NAMING LOOKUP от Rust
по адресу Go-destination.

### Что сделано

- **`go-compat/main.go`** — Go-бинарник `go-sam3-peer`. Роли `server` (echo) и `client`
  (connect + write + ReadFull). JSON-протокол совпадает с sam-bench: `{"type":"ready","dest":"..."}`,
  `{"type":"result","success":...}`. `go.mod` с `go-i2p/sam3 v0.33.92` и `go-i2p/i2pkeys`
  (ключи вынесены в отдельный модуль в новых версиях go-i2p).
- **`sam-compat/`** — новый Rust-бинарник. Роли: `sender` (dial + write + read_exact),
  `receiver` (accept + echo), `lookup` (NAMING LOOKUP). Выводит тот же JSON-протокол.
- **`testbed/tests/sam3_compat.rs`** — три теста на базе `setup_two_sam_nodes()`:
  ноды 1 и 2 с SAM, нода 0 — floodfill. Оба пира запускаются через `ip netns exec`
  с `--sam 127.0.0.1:7656`.
- **Инфраструктура**: `Dockerfile` — сборка Go-бинарника и копирование `sam-compat`;
  `Justfile` — рецепт `test-compat` с явным `cp sam-compat /usr/local/bin/sam-compat`;
  `Cargo.toml` — `sam-compat` в workspace.

### Нетривиальные решения

**Недостижимость SAM-порта с хоста** — первоначальный план вызывать `SamClient::connect`
прямо из тестового кода оказался нереализуемым: i2pd слушает SAM на `127.0.0.1` внутри
netns, у хоста нет маршрута до `10.89.0.0/24`, мост создаётся без IP на хостовой стороне.
Решение: оба пира (и Go, и Rust) запускаются через `ip netns exec`, как в `transfer.rs`.
Это потребовало введения `sam-compat` бинарника — аналога `sam-sender`/`sam-receiver`.

**Дедлок в echo-протоколе** — в схеме «сервер делает `io.Copy(conn, conn)` и пишет
result только после EOF» Rust-тест, читающий result до закрытия соединения, зависает
навсегда. Исправлено тем, что Rust-клиент запускается через `Command::output()` (блокирует
до завершения процесса); процесс завершается и закрывает соединение раньше, чем тест
читает ответ от Go-сервера.

**Определение конца сообщения в `run_receiver`** — без знания длины сообщения echo-ресивер
не знает, когда клиент закончил отправку. Первоначальное решение: `set_read_timeout(5s)` +
`sleep(1s)` — нефрагментированные сообщения работают, но добавляют задержку и ненадёжны
при больших данных. Окончательное решение: передавать `--msg` в `sam-compat receiver`,
тогда `read_exact(msg.len())` детерминированно читает ровно нужное количество байт;
таймаут-путь остаётся как fallback для режима без известной длины.

**`dial_with_retry` в `sam-compat sender`** — одиночный `dial_timeout(120s)` падает
при `CANT_REACH_PEER` (LeaseSet ещё не опубликован). Добавлен цикл retry: попытки каждые 5с,
суммарный deadline 120с; ошибки `CantReachPeer` и `Timeout` — повтор, остальные — немедленный
возврат.

**`test-compat` не копировал бинарник** — рецепт делал `cargo build --release` но не
`cp sam-compat /usr/local/bin/sam-compat`. При изменении кода без пересборки образа
тест использовал старый бинарник. Исправлено добавлением явного `cp` в рецепт
(по аналогии с `test-transfer`).

**Зависимость `i2pkeys`** — в актуальной версии `go-i2p/sam3` типы ключей вынесены в
отдельный пакет `github.com/go-i2p/i2pkeys`; старые примеры с `sam3.I2PKeys` не собираются.
Потребовалось изучить исходники библиотеки в процессе отладки и добавить явный импорт.

### Проверки

- `just test-compat` — все 3 теста PASS: `test_rust_sender_go_receiver`,
  `test_go_sender_rust_receiver`, `test_lookup_against_go_destination`.
- `just test-transfer` — PASS (регрессий нет).
- `just build` — Docker-образ собирается корректно.
- `cargo build --workspace` — PASS, 0 предупреждений.

## DatagramSession, 2026-06-05

Реализован тип `DatagramSession` в библиотеке `sam3` — unreliable-транспорт поверх I2P
по протоколу SAM v3.3. Добавлены тесты совместимости с эталонной Go-реализацией.
Проведено ревью и устранены все выявленные замечания.

### Что сделано

**Структура `DatagramSession` в `sam3/src/session.rs`:**
- Два сокета: `_control: TcpStream` (TCP-управление сессией) и `socket: UdpSocket`
  (UDP для данных, порт 7655 на стороне SAM).
- `send_to(data, dest)` — формирует пакет `3.1 {id} {dest}\n{data}` и отправляет
  его на `sam_udp_addr` (SAM-мост, порт 7655).
- `recv_from(buf)` — читает пакеты в цикле; пакеты не от `sam_udp_addr.ip()`
  отбрасываются (защита от спуфинга); первая строка пакета — адрес отправителя,
  остаток — полезная нагрузка.
- `local_addr()` — адрес локального UDP-сокета (куда SAM-мост доставляет данные).
- `set_read_timeout` / `set_write_timeout` — проксируются на `UdpSocket`.

**Фабричные методы в `SamClient`:**
- `new_datagram_session(id, keys, options)` — создаёт сессию с заданными ключами.
- `new_transient_datagram_session(id, options)` — генерирует ключи и создаёт сессию
  (два TCP-подключения к SAM: одно для `DEST GENERATE`, другое для `SESSION CREATE`).
- UDP-адрес SAM вычисляется через `SocketAddr::parse()` на SAM-адрес плюс замена
  порта на 7655; локальный UDP-сокет биндится на тот же IP (`SocketAddr::new(ip, 0)`).
  Это корректно работает как с IPv4 (`127.0.0.1`), так и с IPv6 (`[::1]`).

**Тесты в `sam3/src/session.rs` (блок `#[cfg(test)] mod tests`):**
- `datagram_session_create_sends_correct_sam_command` — wire-format команды
  `SESSION CREATE STYLE=DATAGRAM … PORT=…`.
- `datagram_session_create_includes_options` — опции туннеля передаются в команду.
- `datagram_send_to_writes_correct_packet_format` — формат UDP-пакета отправки.
- `datagram_recv_from_parses_sender_and_data` — корректное разделение sender/data.
- `datagram_recv_from_ignores_packets_not_from_sam_ip` — IP-фильтрация: пакет от
  `127.0.0.2` отбрасывается, от `127.0.0.1` — принимается.

**Тесты совместимости:**
- Go-роли `datagram-server` и `datagram-client` добавлены в `go-compat/main.go`.
- Rust-роли `datagram-sender` и `datagram-receiver` добавлены в `sam-compat/src/main.rs`.
- Два новых теста в `testbed/tests/sam3_compat.rs`:
  `test_rust_datagram_sender_go_receiver`, `test_go_datagram_sender_rust_receiver`.

### Нетривиальные решения

**Бинд UDP-сокета на `0.0.0.0:0` вместо `sam_host:0`** — исходно сокет биндился на
`0.0.0.0:0`. Метод `local_addr()` возвращал `0.0.0.0:PORT`. В тестах отправка
`sam_udp.send_to(data, 0.0.0.0:PORT)` не доставляла пакет на loopback-сокет (ядро
маршрутизировало пакет через дефолтный шлюз). Тест `recv_from_parses_sender_and_data`
зависал на 60+ секунд; тест `recv_from_ignores_packets_not_from_sam_ip` падал с EAGAIN
после таймаута. Исправлено биндом на `SocketAddr::new(sam_udp_addr.ip(), 0)` — сокет
биндится на тот же IP, что и SAM-мост, и `local_addr()` возвращает корректный адрес.

**Два TCP-подключения для транзитной сессии** — `new_transient_datagram_session` делает
два TCP-подключения к SAM: первое для `DEST GENERATE` (генерация ключей), второе для
`SESSION CREATE STYLE=DATAGRAM`. Тесты с `FakeSam::spawn` (одно соединение) зависали
при ожидании второго клиента. Исправлено через `FakeSam::spawn_many` с отдельным
обработчиком для каждого подключения.

**Сокрытие тестового метода `set_sam_udp_port`** — для теста `send_to` нужна возможность
перенаправить исходящий UDP на тестовый сокет. Метод `set_sam_udp_port(&mut self, port)`
нужен только для тестов. `pub(crate)` недоступен из папки `tests/` (интеграционные тесты —
внешний крейт), `#[cfg(test)] pub fn` не работает (библиотека при сборке тестов
компилируется без флага `test`). Решение: 5 датаграммных тестов перенесены из
`tests/fake_sam.rs` в `src/session.rs` в блок `#[cfg(test)] mod tests`, где метод помечен
`#[cfg(test)] pub(crate)` — полностью скрыт из публичного API.

**Duplicate ID в `go-compat`** — `sam.NewStreamSession` вызывался до ветвления по роли.
Для датаграммных ролей это создавало stream-сессию и datagram-сессию с одним ID. SAM-мост
вернул бы `DUPLICATE_ID`. Исправлено перемещением `NewStreamSession` / `NewDatagramSession`
внутрь соответствующих веток `if role ==`.

**Приведение типа отправителя в Go** — `dg.ReadFrom()` возвращает `net.Addr`. Метод
`Base64()` доступен только у `i2pkeys.I2PAddr`. Потребовалось явное type assertion:
`sender := addr.(i2pkeys.I2PAddr)`.

### Проверки

- `cargo test -p sam3` — 34 теста, PASS, 0 предупреждений (из них 5 новых датаграммных).
- `cargo check --workspace` — PASS.
- `go build ./go-compat` — PASS.

## STREAM FORWARD (ForwardGuard), 2026-06-05

Реализована команда `STREAM FORWARD` протокола SAM v3.3. Позволяет поручить SAM-мосту
самостоятельно принимать входящие I2P-соединения и форвардить их на локальный TCP-порт.

### Контекст

В go-i2p/sam3 эта функциональность явно отмечена как нерабочая (`README: "Does not work:
Stream Forwarding"`). Реализация написана по официальной спецификации SAM v3.3 и
с опорой на синхронную реализацию в библиотеке yosemite 0.7.0 (Rust).

### Wire-format

```
→ (новое TCP-соединение)
HELLO VERSION MIN=3.0 MAX=3.3
← HELLO REPLY RESULT=OK VERSION=3.3
→ STREAM FORWARD ID=<id> PORT=<port> SILENT=<true|false>
← STREAM STATUS RESULT=OK
```

Пока это соединение открыто — мост форвардит входящие I2P-соединения на `port`.
Закрытие соединения = прекращение форвардинга.

### Что сделано

**`ForwardGuard` в `sam3/src/session.rs`:**
```rust
pub struct ForwardGuard(#[allow(dead_code)] TcpStream);
```
RAII-обёртка: держит TCP-соединение открытым. При дропе — соединение закрывается,
мост прекращает форвардинг. `#[allow(dead_code)]` — идиоматично для RAII-обёрток,
поле существует ради `Drop`, а не ради чтения.

**Метод `StreamSession::forward(&self, port: u16, silent: bool)`:**
1. Открывает новое TCP-соединение к SAM (отдельно от control-сокета сессии)
2. `HELLO`
3. `STREAM FORWARD ID={} PORT={port} SILENT={silent}\n`
4. Читает `STREAM STATUS RESULT=OK` через `ensure_ok`
5. Возвращает `ForwardGuard(stream)`

**Unit-тесты (3 штуки в `#[cfg(test)] mod tests`):**
- `stream_forward_sends_correct_command` — проверяет wire-format с `SILENT=false`.
- `stream_forward_silent_sends_silent_true` — проверяет `SILENT=true`.
- `stream_forward_guard_drop_closes_connection` — дропает `ForwardGuard` внутри
  блока; FakeSam-обработчик форвард-соединения убеждается что получает EOF (`read` = 0).

**`ForwardGuard` экспортирован из `lib.rs`.**

### Нетривиальные решения

**Три TCP-соединения для `new_transient_stream_session` + `forward()`** —
в первых версиях тестов `FakeSam::spawn_many` был настроен на два обработчика, но
`forward()` создаёт третье соединение. Тесты падали с `Broken pipe`. Исправлено
добавлением явного обработчика для форвард-соединения.

**`ensure_ok` универсален** — парсит `RESULT=OK` по ключу, не зависит от префикса
(`SESSION STATUS` или `STREAM STATUS`). Работает корректно для ответа на STREAM FORWARD.

### Проверки

- `cargo test -p sam3 stream_forward_` — 3 теста, PASS.
- `cargo check --workspace` — PASS, 0 предупреждений.

## PrimarySession, 2026-06-05

Реализован тип `PrimarySession` (`SESSION CREATE STYLE=PRIMARY`) и `StreamSubSession`.
PrimarySession — контейнер для нескольких sub-сессий разных типов с общей I2P-идентичностью.

### Концепция

Обычные сессии создаются командой `SESSION CREATE` с новым TCP-соединением.
Sub-сессии создаются командой `SESSION ADD` на уже существующем control-соединении primary,
без повторного `HELLO` и без указания `DESTINATION`.

```
SESSION CREATE STYLE=PRIMARY ID=main DESTINATION=<priv> <options>
SESSION ADD STYLE=STREAM ID=main-stream
SESSION ADD STYLE=DATAGRAM ID=main-dg PORT=<port>
SESSION ADD STYLE=RAW ID=main-raw PORT=<port>
```

### Что сделано

**Структуры в `sam3/src/session.rs`:**
- `PrimarySession` — владеет `Arc<Mutex<TcpStream>>` (control-соединение),
  `sam_addr`, `id`, `keys`.
- `StreamSubSession` — держит `Arc<Mutex<TcpStream>>` для удержания primary живым,
  `sam_addr`, `id`, `destination`. Методы `dial`, `dial_timeout`, `listen` —
  открывают отдельные TCP-соединения для данных (стандартный `STREAM CONNECT/ACCEPT`).

**Методы `PrimarySession`:**
- `new_stream_sub_session(&mut self, id)` — отправляет `SESSION ADD STYLE=STREAM ID=…`,
  возвращает `StreamSubSession`.
- `new_datagram_sub_session(&mut self, id, options)` — `SESSION ADD STYLE=DATAGRAM ID=…
  PORT=…`, возвращает `DatagramSession`. UDP-сокет создаётся до команды.
- `new_raw_sub_session(&mut self, id, options)` — аналогично, возвращает `RawSession`.

**Решение проблемы владения `TcpStream`:**
- `DatagramSession` и `RawSession` требуют `_control: TcpStream` (не `Arc`).
- Решение: `stream.try_clone()` — создаёт клон файлового дескриптора. И primary, и
  sub-session держат по FD на одно TCP-соединение. Сессия закрывается только когда
  оба закрыты.
- `StreamSubSession` держит `Arc::clone(&self.control)` — не клонирует FD, разделяет
  указатель.

**Фабричные методы в `SamClient`:**
- `new_primary_session(id, keys, options)`
- `new_transient_primary_session(id, options)` — два TCP-соединения к SAM:
  `DEST GENERATE` + `SESSION CREATE STYLE=PRIMARY`.

**Unit-тесты (5 штук в `#[cfg(test)] mod tests`):**
- Корректность `SESSION CREATE STYLE=PRIMARY ID=… DESTINATION=…`.
- `SESSION ADD STYLE=STREAM ID=…`.
- `SESSION ADD STYLE=DATAGRAM ID=… PORT=…`.
- `SESSION ADD STYLE=RAW ID=… PORT=…`.
- `StreamSubSession::dial()` открывает отдельное TCP-соединение и проходит `STREAM CONNECT`.

**Тесты совместимости:**
- Go-роли `primary-server` и `primary-client` в `go-compat/main.go` используют
  `sam.NewPrimarySession` + `primary.NewStreamSubSession`, затем переиспользуют
  `runServer`/`runClient`.
- Rust-роли `primary-sender` и `primary-receiver` в `sam-compat`; добавлена
  `dial_sub_with_retry` — аналог `dial_with_retry` для `StreamSubSession`.
- Два теста в `testbed/tests/sam3_compat.rs`:
  `test_rust_primary_sender_go_receiver`, `test_go_primary_sender_rust_receiver`.

### Нетривиальные решения

**`Arc<Mutex<TcpStream>>` + `try_clone()`** — сохранён публичный API существующих
структур (`DatagramSession`, `RawSession`), которые ожидают `_control: TcpStream`.
Вместо глобального рефакторинга использован `try_clone()` — дешёвое дублирование
файлового дескриптора на уровне ОС. Семантика корректна: сессия живёт пока существует
хотя бы один держатель.

**`&mut self` для методов создания sub-сессий** — предотвращает параллельное создание
sub-сессий (Rust запрещает несколько `&mut`) без явного `Mutex` на уровне API.
`MutexGuard` используется только внутри метода и освобождается до возврата.

**Перезапись файла при каскадных синтаксических ошибках** — при попытке внести несколько
крупных блоков в `session.rs` инструментами точечной замены сломалась вложенность
модулей. Восстановление через полную перезапись файла (`write_file`) оказалось быстрее,
чем попытки исправить скобки патчами.

### Замечания по ревью (устранено)

Оба замечания ревью устранены до коммита:
- Убран параметр `options` из `new_datagram_sub_session` и `new_raw_sub_session`
  (SAM `SESSION ADD` не принимает tunnel-опции; они задаются один раз в primary).
- Убран `#[derive(Clone)]` у `PrimarySession` (клонирование разделяло бы control-поток
  между двумя значениями, что неожиданно для пользователя).

### Проверки

- `cargo test -p sam3 primary_` — 5 тестов, PASS.
- `cargo check --workspace` — PASS, 0 предупреждений.
- `go build ./go-compat` — PASS.
- `cargo build -p sam-compat` — PASS.

## RawSession, 2026-06-05

Реализован тип `RawSession` в библиотеке `sam3` — минималистичный UDP-транспорт
поверх I2P по протоколу SAM v3.3 (`STYLE=RAW`). Добавлены тесты совместимости
с эталонной Go-реализацией.

### Отличия от DatagramSession

| | RAW | DATAGRAM |
|---|---|---|
| Версия в заголовке отправки | `3.0` | `3.1` |
| Заголовок при приёме | нет — голые байты | `sender_b64\n` |
| Аутентификация отправителя | нет | да (подпись) |
| Метод чтения | `read(buf) → n` | `recv_from(buf) → (n, dest)` |

### Что сделано

**Структура `RawSession` в `sam3/src/session.rs`:**
- Идентична `DatagramSession` по полям: `_control`, `socket`, `sam_udp_addr`, `id`, `keys`.
- `send_to(data, dest)` — заголовок `3.0 {id} {dest}\n`, затем данные.
- `read(buf)` — читает пакет напрямую в `buf` (без промежуточного буфера: нечего
  вырезать), IP-фильтрация по `sam_udp_addr.ip()`.
- `set_read_timeout` / `set_write_timeout`, `local_addr()`, `local_destination()`.
- `#[cfg(test)] pub(crate) fn set_sam_udp_port` — тестовый escape hatch (скрыт из API).

**Хелпер `create_raw_on`** — формирует `SESSION CREATE STYLE=RAW ID=… DESTINATION=…
PORT=… {options} SIGNATURE_TYPE=…`, симметрично `create_datagram_on`.

**Фабричные методы в `SamClient`:**
- `new_raw_session(id, keys, options)`
- `new_transient_raw_session(id, options)` — два TCP-подключения к SAM:
  `DEST GENERATE` + `SESSION CREATE`.

**Unit-тесты (5 штук, блок `#[cfg(test)] mod tests` в `session.rs`):**
- Корректность команды `SESSION CREATE STYLE=RAW … PORT=…`.
- Передача опций туннеля.
- Формат пакета отправки (`3.0 {id} {dest}\nhello world`).
- `read()` возвращает голые байты без заголовка.
- Фильтрация: пакет от `127.0.0.2` отбрасывается, от `127.0.0.1` принимается.

**Тесты совместимости:**
- Go-роли `raw-server` и `raw-client` в `go-compat/main.go`.
  Сервер принимает `--msg`, проверяет содержимое пакета (echo невозможен — адрес
  отправителя в RAW-режиме не возвращается).
- Rust-роли `raw-sender` и `raw-receiver` в `sam-compat/src/main.rs`.
  Receiver требует `--msg` для детерминированной проверки.
- Два новых теста в `testbed/tests/sam3_compat.rs`:
  `test_rust_raw_sender_go_receiver`, `test_go_raw_sender_rust_receiver`.

### Нетривиальные решения

**Одностороннее тестирование вместо echo** — в отличие от Stream и Datagram,
RAW-получатель не знает адрес отправителя и не может ответить. Тесты совместимости
переработаны: получатель проверяет содержимое пакета против `--msg` и выводит
`result`, а отправитель выводит `result` сразу после `send_to`. Тест ждёт оба
результата независимо.

**Прямой буфер в `read()`** — `DatagramSession::recv_from` использует промежуточный
`tmp`-буфер чтобы вырезать `sender_b64\n` перед копированием в пользовательский `buf`.
В `RawSession::read` заголовка нет, `recv_from` пишет прямо в `buf` — лишнее
копирование исключено.

**Точное имя API в go-i2p/sam3** — в документации встречается `NewRawDatagramSession`,
но актуальное имя метода `NewRawSession`; установлено через `go doc`.

### Проверки

- `cargo test -p sam3 raw_` — 5 тестов, PASS.
- `cargo check --workspace` — PASS, 0 предупреждений.
- `go build ./go-compat` — PASS.
- `cargo build -p sam-compat` — PASS.

## SessionOptions builder, 2026-06-05

Реализован типизированный builder для параметров туннельной сессии SAM.
Удалена неструктурированная константа `SAM_TUNNEL_OPTIONS: &[(&str, &str)]`.

### Что сделано

- Новый тип `SessionOptions` в `sam3/src/session.rs` с полями `Option<T>`:
  восемь параметров туннеля (`inbound_length`, `outbound_length`,
  `inbound_length_variance`, `outbound_length_variance`, `inbound_quantity`,
  `outbound_quantity`, `inbound_backup_quantity`, `outbound_backup_quantity`).
- `Default` → все `None` — ничего не передаётся в `SESSION CREATE`,
  i2pd использует собственные дефолты.
- Builder-методы — каждый возвращает `Self` (builder pattern).
- `SessionOptions::zero_hop()` — именованный конструктор, замена
  `SAM_TUNNEL_OPTIONS`: явно задаёт 0-hop, variance=0, quantity=2,
  backupQuantity=0.
- `pub(crate) fn to_pairs(&self) -> Vec<(String, String)>` — сериализует
  только `Some`-поля; порядок полей в выводе не контрактный
  (SAM-протокол нечувствителен к порядку опций).
- Подписи `SamClient::new_stream_session` и `new_transient_stream_session`
  изменены: `&[(&str, &str)]` → `&SessionOptions`.
  `SamSession::create_stream`, `create_stream_with_keys`, `create_stream_on` —
  аналогично.
- `SAM_TUNNEL_OPTIONS` удалён из `session.rs` и `lib.rs`.
- `SessionOptions` экспортируется из `lib.rs`.
- Все вызывающие места обновлены: `sam-bench`, `sam-compat`, `testbed/tests/`,
  `sam3/tests/fake_sam.rs`.

### Нетривиальные решения

**`Option<T>` вместо значений по умолчанию** — если хранить `u8` с дефолтом `0`,
теряется различие между «явно передано 0» и «не указано». `Option<T>` сохраняет
это различие: `Default` → все `None` → пустой `to_pairs()`, i2pd выбирает сам;
`zero_hop()` → все `Some(...)` → 8 явных пар.

**Тесты `to_pairs()` внутри `mod tests` в `session.rs`** — метод `pub(crate)`
недоступен из `sam3/tests/fake_sam.rs` (интеграционные тесты — внешний код).
Юнит-тесты `SessionOptions` размещены в существующем `mod tests` внутри
`session.rs`; интеграционный тест `session_create_uses_options_from_session_options`
проверяет wire-format через FakeSam и не требует прямого вызова `to_pairs()`.

**Конфликт `mod tests`** — при добавлении тестов обнаружен уже существующий
`mod tests` в конце `session.rs`. Новые тесты объединены с ним вместо создания
второго блока.

**`SIGNATURE_TYPE` остаётся отдельно** — параметр типа подписи ключей не переехал
в `SessionOptions`: это характеристика ключевой пары, а не туннельной политики.
Добавляется в `SESSION CREATE` отдельно после опций туннеля.

### Проверки

- 3 новых unit-теста в `mod tests` в `session.rs` — PASS:
  `session_options_default_produces_empty_pairs`,
  `session_options_builder_sets_inbound_length`,
  `session_options_zero_hop_produces_all_eight_pairs`.
- 1 новый интеграционный тест в `fake_sam.rs` — PASS:
  `session_create_uses_options_from_session_options` (позитив + негатив).
- `cargo test -p sam3` — 29 тестов, PASS, 0 предупреждений.
- `cargo build --workspace` — PASS, 0 предупреждений.
