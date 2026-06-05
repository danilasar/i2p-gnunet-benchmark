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
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest bash -lc 'mkdir -p /workspace/tmp/test-artifacts/sam-transfer && cargo build --release --manifest-path /workspace/rs/Cargo.toml && cp /workspace/rs/target/release/sam-sender /usr/local/bin/sam-sender && cp /workspace/rs/target/release/sam-receiver /usr/local/bin/sam-receiver && TEST_ARTIFACT_DIR=/workspace/tmp/test-artifacts/sam-transfer cargo test --manifest-path /workspace/rs/Cargo.toml -p testbed -- test_sam_transfer --nocapture'

# Запустить все Rust интеграционные тесты в Docker
test-rust:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest bash -lc 'cargo test --manifest-path /workspace/rs/Cargo.toml -p testbed -- --test-threads=1 --nocapture'

# Очистка (удаление временных файлов go тестов вне контейнера если есть)
clean:
    rm -rf /tmp/go-build*
