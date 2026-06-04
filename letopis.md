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

Следующий шаг: аналогичный тест для i2pd — две ноды в netns, pre-populated netDb, проверка туннельной связности.
