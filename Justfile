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

# Запустить тест переноса данных SAM
test-transfer:
    docker run --rm --privileged -v $(pwd):/practice -w /practice coursework-overlay:latest bash -lc 'mkdir -p /practice/tmp/test-artifacts/sam-transfer && go build -buildvcs=false -o /usr/local/bin/sam-sender ./cmd/sam-sender && go build -buildvcs=false -o /usr/local/bin/sam-receiver ./cmd/sam-receiver && TEST_ARTIFACT_DIR=/practice/tmp/test-artifacts/sam-transfer go test ./testbed/... -v -run TestSAMTransfer -timeout 20m'

# Очистка (удаление временных файлов go тестов вне контейнера если есть)
clean:
    rm -rf /tmp/go-build*
