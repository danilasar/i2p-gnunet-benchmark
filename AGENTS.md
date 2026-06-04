# AGENTS.md

Инструкции для AI-агентов, работающих с этим репозиторием.

## Контекст

Курсовая работа: методика сравнения транспортных характеристик i2pd/SAM STREAM и GNUnet/CADET.
Рабочая директория: `/home/danilasar/data/areas/studying/coursework/practice/`

## Окружение

- **ОС хоста:** ALT Workstation K 11.3 (Nemorosa), ядро 6.12-alt1
- **Контейнер:** Docker, образ `coursework-overlay:latest` на базе `alt:sisyphus`
  - Сборка: `sudo docker build -t coursework-overlay:latest .` из директории practice/
  - Запуск с сетевой изоляцией: `sudo docker run --rm --privileged -v $(pwd):/practice coursework-overlay:latest bash /practice/<script.sh>`
- **Версии ПО в контейнере:** i2pd 2.60.0, GNUnet 0.26.2, Python 3.x с networkx

## Правила логирования

- Все действия (успешные и неудачные) фиксировать в `practice/letopis.md`
- Ошибки, тупиковые пути, изменения подхода — фиксировать явно с объяснением
- Git-коммиты: стиль `english_tag: русскоязычное описание`
- Коммитить при каждом значимом результате

## Структура репозитория

```
Dockerfile          — образ coursework-overlay (alt:sisyphus + gnunet + i2pd)
letopis.md          — хронология всех действий
ri_to_netdb.py      — конвертация router.info → netDb/XX/routerInfo-<hash>.dat
smoke_gnunet.sh     — smoke-тест GNUnet: 2 пира в netns, CORE-соединение
smoke_i2pd.sh       — smoke-тест i2pd: 4 ноды в netns, DHT exploration через floodfill
AGENTS.md           — этот файл
```

## Известные особенности

### GNUnet 0.26.2
- Нет `gnunet-peerinfo` — используются `gnunet-hello`, `gnunet-statistics`
- Transport через communicators (`gnunet-communicator-tcp`), не plugins
- Конфиги требуют `@INLINE@` для стандартных настроек из `/usr/share/gnunet/config.d/`
- Обмен HELLO: `gnunet-hello -e` (экспорт), `gnunet-hello --import` (импорт)
- Проверка CORE-соединения: поиск "notification about connection from" в логах

### i2pd 2.60.0
- `--reseed.urls=` (пустое значение) не работает через CLI — только через conf-файл
- `[reseed] threshold = 0` отключает ВСЁ (включая ZIP reseed)
- RouterInfo: два поля `caps` — в адресном блоке (~offset 418, "4") и глобальном (~offset 492, "Xf" для floodfill)
- Floodfill bootstrap: нужна хотя бы одна нода с `--floodfill` + ZIP reseed для остальных
- Bootstrap через `[reseed] zipfile = /path/to.zip; threshold = 50`
- Время bootstrap: ~55 секунд до первого DHT exploration
- Детекция связности: `NetDbReq: Exploring new N routers` в логе
- `ri_to_netdb.py` вычисляет SHA256(RouterIdentity) → правильное имя файла для netDb

## Запуск smoke-тестов

```bash
# GNUnet
sudo docker run --rm --privileged -v $(pwd):/practice \
    coursework-overlay:latest bash /practice/smoke_gnunet.sh

# i2pd
sudo docker run --rm --privileged -v $(pwd):/practice \
    coursework-overlay:latest bash /practice/smoke_i2pd.sh
```

Ожидаемый результат:
- GNUnet: `[PASS] Peers установили CORE-соединение!`
- i2pd: `[PASS] 4/4 нод активны в DHT exploration (соединения установлены)!`
