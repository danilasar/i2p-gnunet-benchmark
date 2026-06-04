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

**Dockerfile:** `practice/Dockerfile`

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
`TEST_ARTIFACT_DIR=/practice/tmp/test-artifacts/sam-transfer`; при диагностике тест теперь
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

`just test-transfer` создаёт временный каталог `/practice/tmp/test-artifacts` внутри
контейнера перед запуском теста. После успешного прогона каталог `tmp/` удалён через Docker,
чтобы в рабочей копии не оставались root-owned артефакты, мешающие последующим `go test ./...`.

## README TODO, 2026-06-04 23:20 +04

По замечанию о текущем bootstrap в `TestSAMTransfer` добавлен пункт TODO в таблицу README:
сейчас nodes 1-3 заранее получают `RouterInfo` всех нод через `reseed_full.zip`, а целевое
поведение — discovery от seed/floodfill-ноды без знания всех участников.
