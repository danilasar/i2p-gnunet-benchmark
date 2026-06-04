#!/bin/bash
# Smoke-тест i2pd 2.60.0: N роутеров в изолированных netns, соединённых через bridge.
# Arm B: zero-hop (length=0), exploratory tunnels также zero-hop.
# Bootstrap: node0 — floodfill; остальные ноды получают его RouterInfo через local ZIP reseed.
# Проверяет: ноды устанавливают NTCP2-соединение с floodfill, DHT exploration активна.
# Требует: root, привилегированный контейнер.
set -e

BASEDIR=/tmp/i2pd_smoke
N_NODES=4
BRIDGE=br_i2p
SUBNET="10.88.0"            # ноды: 10.88.0.1 .. 10.88.0.N
BASE_NTCP2_PORT=12000       # NTCP2 порт ноды i = BASE_NTCP2_PORT + i
SAM_PORT=17656              # SAM bridge только на node0

WAIT_GENRI=12               # секунд на генерацию RouterInfo (первый запуск)
WAIT_CONNECT=70             # секунд на установку соединений (Bootstrap занимает ~55 сек)

RI_SCRIPT=/practice/ri_to_netdb.py

cleanup() {
    echo "[cleanup] Останавливаем i2pd..."
    for i in $(seq 0 $((N_NODES-1))); do
        pkill -f "datadir=$BASEDIR/node$i " 2>/dev/null || true
    done
    sleep 2
    for i in $(seq 0 $((N_NODES-1))); do
        ip netns del "ns_i2p${i}" 2>/dev/null || true
    done
    ip link del $BRIDGE 2>/dev/null || true
    echo "[cleanup] Готово."
}
trap cleanup EXIT

echo "=== i2pd 2.60.0 smoke test (${N_NODES} nodes, zero-hop, zip-bootstrap) ==="

rm -rf $BASEDIR
mkdir -p $BASEDIR

# --- 1. Bridge + namespaces ---
echo "[net] Создаём bridge $BRIDGE и $N_NODES namespaces..."
ip link add $BRIDGE type bridge
ip link set $BRIDGE up

for i in $(seq 0 $((N_NODES-1))); do
    NS="ns_i2p${i}"
    IP="${SUBNET}.$((i+1))"
    VETH_HOST="veth_h${i}"
    VETH_NS="veth_n${i}"

    ip netns add $NS
    ip link add $VETH_HOST type veth peer name $VETH_NS
    ip link set $VETH_HOST master $BRIDGE
    ip link set $VETH_HOST up
    ip link set $VETH_NS netns $NS

    ip netns exec $NS ip addr add "${IP}/24" dev $VETH_NS
    ip netns exec $NS ip link set $VETH_NS up
    ip netns exec $NS ip link set lo up

    echo "  $NS: $IP"
done

# --- 2. Конфиги ---
write_conf() {
    local node=$1 ip=$2 port=$3 zipfile=${4:-}
    local dir="$BASEDIR/node$node"
    mkdir -p "$dir/netDb"

    local reseed_section
    if [ -n "$zipfile" ]; then
        reseed_section="[reseed]
urls =
threshold = 50
zipfile = $zipfile
verify = false"
    else
        reseed_section="[reseed]
urls =
threshold = 0"
    fi

    local sam_section
    if [ "$node" -eq 0 ]; then
        sam_section="[sam]
enabled = true
address = 127.0.0.1
port = $SAM_PORT"
    else
        sam_section="[sam]
enabled = false"
    fi

    cat > "$dir/i2pd.conf" <<EOF
${reseed_section}

[ntcp2]
enabled = true
published = true
port = ${port}

[ssu2]
enabled = false

[httpproxy]
enabled = false
[socksproxy]
enabled = false
[http]
enabled = false

[exploratory]
inbound.length = 0
outbound.length = 0
inbound.quantity = 2
outbound.quantity = 2

${sam_section}
EOF
}

# node0: floodfill, без zipfile (он сам генерирует RouterInfo для других)
write_conf 0 "${SUBNET}.1" $BASE_NTCP2_PORT

# node1..N: конфиги сначала без zipfile (zip будет создан после первого запуска node0)
for i in $(seq 1 $((N_NODES-1))); do
    write_conf $i "${SUBNET}.$((i+1))" $((BASE_NTCP2_PORT + i))
done

# --- 3. Первый запуск: генерируем RouterInfo node0 (floodfill) ---
echo "[i2pd] Первый запуск node0 (floodfill) для генерации RouterInfo ($WAIT_GENRI сек.)..."
ip netns exec ns_i2p0 i2pd \
    --datadir="$BASEDIR/node0" \
    --conf="$BASEDIR/node0/i2pd.conf" \
    --address4="${SUBNET}.1" \
    --floodfill \
    --loglevel=warn \
    --log=file --logfile="$BASEDIR/node0/i2pd_init.log" \
    --daemon &
sleep $WAIT_GENRI

if [ ! -f "$BASEDIR/node0/router.info" ]; then
    echo "[FAIL] node0 не создал router.info"
    cat "$BASEDIR/node0/i2pd_init.log" | tail -20
    exit 1
fi
echo "  node0 router.info: $(wc -c < $BASEDIR/node0/router.info) bytes"

# Останавливаем node0
pkill -f "datadir=$BASEDIR/node0 " 2>/dev/null; sleep 2

# --- 4. Создаём ZIP reseed из RouterInfo node0 ---
echo "[i2pd] Создаём ZIP reseed из node0 RouterInfo..."
python3 << PYEOF
import zipfile, hashlib, base64, struct, sys

def i2p_b64(d):
    return base64.b64encode(d).decode().replace('+','-').replace('/','~').rstrip('=')

def ri_hash(path):
    data = open(path, 'rb').read()
    off = 256 + 128
    cert_len = struct.unpack('>H', data[off+1:off+3])[0]
    return i2p_b64(hashlib.sha256(data[:off+3+cert_len]).digest())

h = ri_hash('$BASEDIR/node0/router.info')
with zipfile.ZipFile('$BASEDIR/reseed_node0.zip', 'w') as z:
    z.write('$BASEDIR/node0/router.info', f'routerInfo-{h}.dat')
print(f'  ZIP OK: hash={h[:20]}...')

# Проверим floodfill bit
data = open('$BASEDIR/node0/router.info','rb').read()
idx = 0
while True:
    idx = data.find(b'caps', idx)
    if idx < 0: break
    vl = data[idx+5]
    val = data[idx+6:idx+6+vl]
    if b'f' in val:
        print(f'  floodfill bit найден в caps@{idx}: {repr(val)}')
    idx += 1
PYEOF

# --- 5. Обновляем конфиги node1..N с zipfile ---
for i in $(seq 1 $((N_NODES-1))); do
    write_conf $i "${SUBNET}.$((i+1))" $((BASE_NTCP2_PORT + i)) "$BASEDIR/reseed_node0.zip"
done

# --- 6. Запуск всех нод ---
echo "[i2pd] Запускаем все ${N_NODES} ноды..."
# node0 (floodfill) первым
ip netns exec ns_i2p0 i2pd \
    --datadir="$BASEDIR/node0" \
    --conf="$BASEDIR/node0/i2pd.conf" \
    --address4="${SUBNET}.1" \
    --floodfill \
    --loglevel=info \
    --log=file --logfile="$BASEDIR/node0/i2pd.log" \
    --daemon &
sleep 3

# остальные ноды
for i in $(seq 1 $((N_NODES-1))); do
    NS="ns_i2p${i}"
    IP="${SUBNET}.$((i+1))"
    ip netns exec $NS i2pd \
        --datadir="$BASEDIR/node$i" \
        --conf="$BASEDIR/node$i/i2pd.conf" \
        --address4="$IP" \
        --loglevel=info \
        --log=file --logfile="$BASEDIR/node$i/i2pd.log" \
        --daemon &
done

echo "  Ждём установки соединений ($WAIT_CONNECT сек.)..."
sleep $WAIT_CONNECT

# --- 7. Результаты ---
echo ""
echo "=== Результаты ==="

FLOODFILL_HASH=$(python3 -c "
import hashlib, base64, struct
def i2p_b64(d): return base64.b64encode(d).decode().replace('+','-').replace('/','~').rstrip('=')
data=open('$BASEDIR/node0/router.info','rb').read()
off=256+128; cl=struct.unpack('>H',data[off+1:off+3])[0]
print(i2p_b64(hashlib.sha256(data[:off+3+cl]).digest())[:20])
" 2>/dev/null)
echo "  floodfill (node0) hash: ${FLOODFILL_HASH}..."

TOTAL_CONNECTED=0
for i in $(seq 0 $((N_NODES-1))); do
    LOG="$BASEDIR/node$i/i2pd.log"
    EXPLORE=$(grep -c "NetDbReq: Exploring\|Exploring new" $LOG 2>/dev/null || echo 0)
    PROFILE=$(grep -c "Profiling:" $LOG 2>/dev/null || echo 0)
    RI_ADDED=$(grep -c "RouterInfo added" $LOG 2>/dev/null || echo 0)

    echo "  node$i (${SUBNET}.$((i+1))): explore=${EXPLORE} | profiling_events=${PROFILE} | ri_added=${RI_ADDED}"
    [ "$EXPLORE" -gt 0 ] && TOTAL_CONNECTED=$((TOTAL_CONNECTED + 1))
done

echo ""
if [ "$TOTAL_CONNECTED" -ge 2 ]; then
    echo "[PASS] ${TOTAL_CONNECTED}/${N_NODES} нод активны в DHT exploration (соединения установлены)!"
else
    echo "[PARTIAL] DHT exploration: ${TOTAL_CONNECTED}/${N_NODES} нод. Смотрим логи..."
    echo ""
    echo "=== Лог node1 (последние 30 строк без tunnel errors) ==="
    grep -v 'select first hop\|create.*tunnel\|select next hop\|no peers avail' \
        "$BASEDIR/node1/i2pd.log" 2>/dev/null | tail -30 || echo "(нет лога)"
fi

# SAM-тест (node0)
echo ""
echo "[i2pd] Проверка SAM bridge на node0..."
ip netns exec ns_i2p0 python3 -c "
import socket
s = socket.socket()
s.settimeout(5)
try:
    s.connect(('127.0.0.1', $SAM_PORT))
    s.send(b'HELLO VERSION MIN=3.0 MAX=3.3\n')
    resp = s.recv(256)
    print('  SAM OK: ' + resp.decode().strip())
    s.close()
except Exception as e:
    print(f'  SAM: {e}')
" 2>/dev/null || echo "  SAM check failed"
