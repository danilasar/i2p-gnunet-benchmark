# Руководство экспериментатора

Этот документ описывает, как правильно провести измерение на стенде,
какие параметры выбрать и как интерпретировать результаты.

## Что измеряется

Стенд сравнивает **надёжную передачу полезной нагрузки между двумя
application endpoints поверх overlay** в трёх конфигурациях:

| Arm | Система | Конфигурация | Security model |
|-----|---------|--------------|----------------|
| A | i2pd / SAM STREAM | `inbound.length=3, outbound.length=3` | Onion routing, анонимность источника и получателя |
| B | i2pd / SAM STREAM | `inbound.length=0, outbound.length=0` | E2E-шифрование, нет anonymity routing |
| C | GNUnet / CADET | reliable channel | E2E-шифрование, DHT routing |

Arm B и Arm C структурно сопоставимы по отсутствию anonymity routing.
Разница между ними — криптостек, механизм discovery и transport layer.
Разница между Arm A и Arm B — измеримая стоимость anonymity routing внутри i2pd.

## Метрики

| Группа | Метрика | Единицы |
|--------|---------|---------|
| Overlay | `setup_ms` — время до готовности канала | мс |
| Overlay | `first_byte_ms` — до первого байта payload | мс |
| Overlay | `transfer_ms` — полное время передачи | мс |
| Overlay | `goodput_mbps` — полезная скорость | Мбит/с |
| Overlay | `overlay_rtt_p50/p95_ms` | мс |
| Overlay | `timeout`, `abort` — неуспешные прогоны | bool |
| Underlay | `rtt_p50/p95_ms` между нодами | мс |
| Host | `cpu_avg_pct`, `rss_peak_mib` | % / МиБ |

**Не raw throughput, а goodput:** `delivered_payload_bytes / transfer_ms`.

## Уровни стенда

| Tier | Назначение | Статус |
|------|-----------|--------|
| Tier 1 | Controlled local stand: netns + veth + tc/netem | Основная доказательная база |
| Tier 2 | VPS в разных AS — калибровка underlay-профилей | Supporting evidence |
| Tier 3 | Публичная сеть — sanity check | Наблюдательный материал |

Для курсовой работы достаточен Tier 1.

## Underlay-профили

Профили задаются через `tc qdisc netem` на veth-интерфейсах и хранятся
в конфигурации топологии.

| Профиль | RTT | Loss | Jitter | Bandwidth | Назначение |
|---------|-----|------|--------|-----------|-----------|
| P1 | низкий | нет | нет | высокий | baseline |
| P2 | средний | нет | нет | 50 Мбит/с | типичный LAN |
| P3 | высокий | нет | нет | — | WAN latency |
| P4 | средний | нет | высокий | — | нестабильный канал |
| P5 | средний | высокий | — | — | lossy relay |
| P6 | средний | нет | нет | ограниченный | узкое место |
| P7 | — | — | — | — | churn: relay выпадает |

## Выбор размеров нагрузки

Не назначать фиксированные 1/10/100 МиБ. Алгоритм:

1. Запустить 10–15 **pilot runs** с произвольным размером (например, 5 МиБ).
2. Получить `R_pilot` — медиана goodput по пилоту.
3. Выбрать размеры:
   - **Small:** `transfer_time ≈ 1–3 × setup_time`
   - **Medium:** `S = R_pilot × 10–30 с`
   - **Large:** `S = R_pilot × 60–180 с`

Это гарантирует, что large-прогон измеряет steady-state goodput,
а small-прогон — стоимость старта соединения.

## Порядок проведения измерений

### 1. Подготовка

```bash
just build          # собрать образ с актуальными версиями ПО
just test           # убедиться что smoke-тесты проходят
```

Зафиксировать версии:
```bash
docker run --rm coursework-overlay:latest bash -c \
  "i2pd --version; gnunet-arm --version"
```

### 2. Настройка прогона

Все параметры задаются через `testbed/` — конфигурация топологии, arm, профиль.
Каждый прогон должен иметь уникальный `run_id` в формате:

```
tier1_<profile>_<arm>_<workload>_<warm|cold>_seed<N>_<seq>
```

Пример: `tier1_p2_i2pd_std_medium_warm_seed42_00031`

### 3. Warm vs Cold

- **Cold:** канал строится заново, leaseSet/путь не закеширован.
- **Warm:** overlay уже присоединился к сети, но сам transfer запускается отдельно.

Измерять оба режима. `setup_ms` в cold включает время построения пути.

### 4. Сбор логов

Каждый прогон записывает JSONL-строку. Обязательные поля:

```json
{
  "run_id": "...",
  "arm_id": "i2pd_standard|i2pd_e2e_only|gnunet_cadet",
  "tunnel_length": 3,
  "underlay_profile": "P2",
  "warm_state": "cold",
  "workload_class": "medium",
  "file_size_bytes": 41943040,
  "setup_ms": 842.7,
  "transfer_ms": 20014.0,
  "goodput_mbps": 16.76,
  "success": true,
  "sha256_ok": true
}
```

### 5. Воспроизводимость

Перед публикацией результатов убедиться, что сохранены:
- git commit hash стенда
- конфиги i2pd и GNUnet (hash)
- seed топологии
- underlay matrix (netem-параметры)
- raw JSONL-логи
- версии ПО

## Число прогонов

| Цель | Число |
|------|-------|
| Pilot (оценка дисперсии) | 10–15 |
| Goodput / median CI | после пилота, до стабилизации CI |
| Latency p95 | ≥ 500 samples |
| Latency p99 | ≥ 5 000 samples |
| Timeout/abort rate 95% CI | ≥ 200 attempts |

## Анализ результатов

```bash
cd analysis
pip install -r requirements.txt
python analyzer.py --input logs/run_*.jsonl --output results/
```

Отчёт включает: CDF setup_time, CDF overlay_rtt, boxplot goodput по профилям,
таблицу success/timeout/abort, таблицу CPU/RAM.

Всегда указывать p50, p95, p99 и bootstrap confidence intervals.
Среднее значение само по себе не является результатом.

## Типичные ошибки

- **Смешивать warm и cold** в одной выборке — setup_time будет биmodal.
- **Забыть проверить SHA-256** — неполная передача засчитается как успех.
- **Ждать слишком мало** после запуска нод — i2pd требует ~55 с на bootstrap,
  GNUnet — ~10 с до стабильного CORE.
- **Сравнивать Arm A с Arm C напрямую** без учёта разницы в security model —
  часть разницы в goodput объясняется разным cryptographic overhead, а не эффективностью реализации.
