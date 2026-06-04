set shell := ["bash", "-c"]

# Собрать образ для тестов
build:
    docker build -t coursework-overlay:latest .

# Запустить все smoke-тесты в Docker
test:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest go test ./testbed/... -v -run Smoke -timeout 5m

# Запустить только GNUnet smoke
test-gnunet:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest go test ./testbed/... -v -run TestGnunetSmoke -timeout 2m

# Запустить только I2P smoke
test-i2p:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest go test ./testbed/... -v -run TestI2pdSmoke -timeout 3m

# Сборка Rust бинарников
build-rs:
    cd rs && cargo build

# Очистка (удаление временных файлов go тестов вне контейнера если есть)
clean:
    rm -rf /tmp/go-build*
