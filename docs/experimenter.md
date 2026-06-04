# Руководство экспериментатора

## Что исследуется

Стенд сравнивает **надёжную передачу payload между двумя application
endpoints поверх overlay** в трёх конфигурациях:

| Arm | Система | Конфигурация |
|-----|---------|--------------|
| A | i2pd / SAM STREAM | `inbound.length=3, outbound.length=3` — стандартный onion routing |
| B | i2pd / SAM STREAM | `inbound.length=0, outbound.length=0` — только E2E, без анонимизации |
| C | GNUnet / CADET | reliable channel — DHT routing, E2E шифрование |

Arm B и C сопоставимы по отсутствию anonymity routing.
Разница A − B = стоимость anonymity routing в i2pd.
Разница B − C = разница транспортных стеков при одинаковом security context.

Метрики: setup time, first-byte time, overlay RTT, goodput, timeout/abort rate,
CPU/RAM.

## Что реализовано сейчас

**Smoke-тесты** проверяют, что инфраструктура работает:

- `TestGnunetSmoke` — два пира в изолированных netns обмениваются HELLO
  и устанавливают CORE-соединение (~15 с).
- `TestI2pdSmoke` — четыре ноды bootstrapped через локальный ZIP reseed,
  выполняют DHT exploration, SAM bridge отвечает (~90 с).

Это не измерения производительности — это проверка работоспособности стенда.

## Что не реализовано (запланировано)

- Sender/receiver для SAM STREAM (Arm A, B) — Rust, `rs/sam-sender`, `rs/sam-receiver`
- Sender/receiver для CADET (Arm C)
- Наложение underlay-профилей через `tc netem`
- Сбор метрик (goodput, RTT, CPU/RSS) и запись в JSONL
- Pilot-запуски и вывод численных параметров
- Анализ результатов (`analysis/`)

## Запуск текущих тестов

```bash
just build        # собрать образ (один раз)
just test         # оба smoke-теста
just test-gnunet  # только GNUnet
just test-i2p     # только i2pd
```

## Топология стенда

```
Linux bridge (br_*)
  ├── veth → ns_*0  (10.X.0.1)  нода 0
  ├── veth → ns_*1  (10.X.0.2)  нода 1
  └── ...
```

Все ноды изолированы от внешней сети и от хоста.
Связь только между нодами через bridge внутри контейнера.

## i2pd: особенности bootstrap

i2pd не умеет работать в закрытой сети без reseed-сервера.
Решение: нода 0 запускается как **floodfill**, её `router.info`
упаковывается в ZIP, остальные ноды загружают его через `[reseed] zipfile`.

Время до первого DHT exploration — около 55 секунд.
Таймаут в `TestI2pdSmoke` — 90 секунд.

## GNUnet: особенности bootstrap

Ноды обмениваются HELLO-файлами вручную через `gnunet-hello -e` / `--import`.
После импорта HELLO-файла GNUnet устанавливает CORE-соединение за 5–15 секунд.

## Планируемая методология измерений

> Этот раздел описывает будущее состояние стенда.

Размеры нагрузки не фиксируются заранее. После реализации sender/receiver:

1. Запустить 10–15 pilot runs, получить медиану goodput `R_pilot`.
2. Выбрать размеры:
   - **Small:** `transfer_time ≈ 1–3 × setup_time` (измеряет стоимость старта)
   - **Medium:** `S = R_pilot × 10–30 с`
   - **Large:** `S = R_pilot × 60–180 с` (steady-state goodput)

Каждый прогон фиксируется в JSONL с полями: `run_id`, `arm_id`,
`underlay_profile`, `warm_state`, `workload_class`, `setup_ms`,
`transfer_ms`, `goodput_mbps`, `success`, `sha256_ok`.

Результаты представляются через p50/p95/p99 и bootstrap CI, не средним.
