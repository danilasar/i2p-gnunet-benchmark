#!/bin/bash
# Smoke-тест GNUnet 0.26: два пира в изолированных network namespaces.
# Проверяет, что peers устанавливают CORE-соединение и видят друг друга.
# Требует: запуск с правами root внутри привилегированного контейнера.
set -e

BASEDIR=/tmp/gnunet_smoke
PEER0=$BASEDIR/peer0
PEER1=$BASEDIR/peer1

NS0=ns_gn0
NS1=ns_gn1

IP0=10.99.0.1
IP1=10.99.0.2
PREFIX=24

# Порты communicator-tcp для каждого пира
PORT0=2086
PORT1=2087

WAIT_BOOTSTRAP=10  # секунд на старт сервисов
WAIT_CONNECT=15    # секунд на установку соединения

cleanup() {
    echo "[cleanup] Останавливаем GNUnet..."
    ip netns exec $NS0 env GNUNET_HOME=$PEER0 gnunet-arm -c $PEER0/peer.conf -e 2>/dev/null || true
    ip netns exec $NS1 env GNUNET_HOME=$PEER1 gnunet-arm -c $PEER1/peer.conf -e 2>/dev/null || true
    sleep 2
    ip netns del $NS0 2>/dev/null || true
    ip netns del $NS1 2>/dev/null || true
    echo "[cleanup] Готово."
}
trap cleanup EXIT

echo "=== GNUnet 0.26 smoke test ==="

# --- 1. Рабочие директории ---
rm -rf $BASEDIR
mkdir -p $PEER0/.cache/gnunet $PEER0/data/hosts \
         $PEER1/.cache/gnunet $PEER1/data/hosts

# --- 2. Network namespaces + veth ---
echo "[net] Создаём namespaces и veth-пару..."
ip netns add $NS0
ip netns add $NS1

ip link add veth0 type veth peer name veth1
ip link set veth0 netns $NS0
ip link set veth1 netns $NS1

ip netns exec $NS0 ip addr add $IP0/$PREFIX dev veth0
ip netns exec $NS0 ip link set veth0 up
ip netns exec $NS0 ip link set lo up

ip netns exec $NS1 ip addr add $IP1/$PREFIX dev veth1
ip netns exec $NS1 ip link set veth1 up
ip netns exec $NS1 ip link set lo up

ip netns exec $NS0 ip addr show veth0 | grep -q "$IP0" && echo "  veth0 ($IP0) OK"
ip netns exec $NS1 ip addr show veth1 | grep -q "$IP1" && echo "  veth1 ($IP1) OK"

# --- 3. Конфиги GNUnet 0.26 ---
write_conf() {
    local dir=$1 ip=$2 port=$3
    mkdir -p $dir
    cat > $dir/peer.conf <<EOF
# Подключаем стандартные конфиги GNUnet
@INLINE@ /usr/share/gnunet/config.d/arm.conf
@INLINE@ /usr/share/gnunet/config.d/transport.conf
@INLINE@ /usr/share/gnunet/config.d/core.conf
@INLINE@ /usr/share/gnunet/config.d/cadet.conf
@INLINE@ /usr/share/gnunet/config.d/dht.conf
@INLINE@ /usr/share/gnunet/config.d/peerstore.conf
@INLINE@ /usr/share/gnunet/config.d/statistics.conf
@INLINE@ /usr/share/gnunet/config.d/hostlist.conf

[PATHS]
GNUNET_HOME = $dir
GNUNET_RUNTIME_DIR = $dir/run
GNUNET_USER_RUNTIME_DIR = $dir/run
GNUNET_CACHE_HOME = $dir/.cache/gnunet

[arm]
OPTIONS = -l $dir/.cache/gnunet/gnunet.log

[hostlist]
# Отключаем внешний bootstrap
SERVERS =
HTTPPORT = 0

[communicator-tcp]
BINDTO = $ip:$port

[communicator-udp]
# Отключаем UDP (упрощаем smoke-тест)
IMMEDIATE_START = NO

[nat]
# Нет NAT в netns
ENABLE_UPNP = NO
EOF
}

write_conf $PEER0 $IP0 $PORT0
write_conf $PEER1 $IP1 $PORT1

# --- 4. Запуск peers ---
echo "[gnunet] Запускаем peer0 ($IP0:$PORT0)..."
ip netns exec $NS0 env GNUNET_HOME=$PEER0 gnunet-arm -c $PEER0/peer.conf -s 2>/dev/null &
sleep 2

echo "[gnunet] Запускаем peer1 ($IP1:$PORT1)..."
ip netns exec $NS1 env GNUNET_HOME=$PEER1 gnunet-arm -c $PEER1/peer.conf -s 2>/dev/null &

echo "[gnunet] Ожидаем инициализацию ($WAIT_BOOTSTRAP сек.)..."
sleep $WAIT_BOOTSTRAP

# --- 5. Получаем HELLO ---
echo "[gnunet] Экспортируем HELLO..."
PEER0_HELLO=$(ip netns exec $NS0 env GNUNET_HOME=$PEER0 gnunet-hello -c $PEER0/peer.conf -e 2>/dev/null) || PEER0_HELLO=""
PEER1_HELLO=$(ip netns exec $NS1 env GNUNET_HOME=$PEER1 gnunet-hello -c $PEER1/peer.conf -e 2>/dev/null) || PEER1_HELLO=""

echo "  peer0 HELLO: ${PEER0_HELLO:0:60}..."
echo "  peer1 HELLO: ${PEER1_HELLO:0:60}..."

if [ -z "$PEER0_HELLO" ] || [ -z "$PEER1_HELLO" ]; then
    echo "[WARN] HELLO пусто — проверяем лог..."
    cat $PEER0/.cache/gnunet/gnunet.log 2>/dev/null | tail -20
    echo "[INFO] Пробуем продолжить..."
fi

# --- 6. Обмен HELLO через файлы ---
echo "[gnunet] Импортируем HELLO между пирами..."
if [ -n "$PEER0_HELLO" ]; then
    echo "$PEER0_HELLO" | ip netns exec $NS1 env GNUNET_HOME=$PEER1 \
        gnunet-hello -c $PEER1/peer.conf --import 2>/dev/null \
        && echo "  peer0 HELLO -> peer1: OK" || echo "  peer0 HELLO -> peer1: skip"
fi
if [ -n "$PEER1_HELLO" ]; then
    echo "$PEER1_HELLO" | ip netns exec $NS0 env GNUNET_HOME=$PEER0 \
        gnunet-hello -c $PEER0/peer.conf --import 2>/dev/null \
        && echo "  peer1 HELLO -> peer0: OK" || echo "  peer1 HELLO -> peer0: skip"
fi

echo "[gnunet] Ожидаем установку соединения ($WAIT_CONNECT сек.)..."
sleep $WAIT_CONNECT

# --- 7. Проверка через gnunet-statistics ---
echo "[gnunet] Проверяем CORE neighbours..."
CORE_NEIGHBOURS_0=$(ip netns exec $NS0 env GNUNET_HOME=$PEER0 \
    gnunet-statistics -c $PEER0/peer.conf -q -s core 2>/dev/null | grep -i "neighbour\|connect" || echo "")
CORE_NEIGHBOURS_1=$(ip netns exec $NS1 env GNUNET_HOME=$PEER1 \
    gnunet-statistics -c $PEER1/peer.conf -q -s core 2>/dev/null | grep -i "neighbour\|connect" || echo "")

echo "  peer0 core stats: $CORE_NEIGHBOURS_0"
echo "  peer1 core stats: $CORE_NEIGHBOURS_1"

# --- 8. Проверка через gnunet-hello -D ---
echo "[gnunet] Известные HELLOs у peer0:"
ip netns exec $NS0 env GNUNET_HOME=$PEER0 gnunet-hello -c $PEER0/peer.conf -D 2>/dev/null || echo "  (пусто)"
echo "[gnunet] Известные HELLOs у peer1:"
ip netns exec $NS1 env GNUNET_HOME=$PEER1 gnunet-hello -c $PEER1/peer.conf -D 2>/dev/null || echo "  (пусто)"

# --- 9. Итог: ищем CORE-соединение в логах ---
echo ""
echo "=== Итог ==="
CORE_CONNECT_0=$(grep -c "notification about connection from" $PEER0/.cache/gnunet/gnunet.log 2>/dev/null || echo "0")
CORE_CONNECT_1=$(grep -c "notification about connection from" $PEER1/.cache/gnunet/gnunet.log 2>/dev/null || echo "0")
echo "  CORE connect events peer0: $CORE_CONNECT_0"
echo "  CORE connect events peer1: $CORE_CONNECT_1"

if [ "$CORE_CONNECT_0" -gt 0 ] || [ "$CORE_CONNECT_1" -gt 0 ]; then
    echo "[PASS] Peers установили CORE-соединение!"
    grep "notification about connection" $PEER0/.cache/gnunet/gnunet.log 2>/dev/null | head -3
else
    echo "[FAIL] CORE-соединение не зафиксировано в логах."
    echo ""
    echo "=== Логи peer0 (последние 30 строк) ==="
    tail -30 $PEER0/.cache/gnunet/gnunet.log 2>/dev/null || echo "(нет лога)"
fi
