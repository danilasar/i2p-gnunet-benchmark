set shell := ["bash", "-c"]

# Собрать образ для тестов
build:
    docker build -t coursework-overlay:latest .

# Быстрые тесты без Docker и root
test-unit:
    cargo test -p sam3 -p sam-sender -p sam-receiver && cargo test -p testbed --lib

# Быстрые тесты SAM-библиотеки с fake SAM server
test-sam3:
    cargo test -p sam3 -- --nocapture

# Запустить все smoke-тесты в Docker
test:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest bash -lc 'cargo build --release --manifest-path /workspace/Cargo.toml && cp /workspace/target/release/sam-sender /usr/local/bin/sam-sender && cp /workspace/target/release/sam-receiver /usr/local/bin/sam-receiver && cargo test --manifest-path /workspace/Cargo.toml -p testbed -- --test-threads=1 --nocapture'

# Запустить только GNUnet smoke
test-gnunet:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest cargo test --manifest-path /workspace/Cargo.toml -p testbed -- test_gnunet_smoke --nocapture

# Запустить только I2P smoke
test-i2p:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest cargo test --manifest-path /workspace/Cargo.toml -p testbed -- test_i2pd_smoke --nocapture

# Запустить тест переноса данных SAM
test-transfer:
    docker run --rm --privileged -v $(pwd):/workspace -w /workspace coursework-overlay:latest bash -lc 'mkdir -p /workspace/tmp/test-artifacts/sam-transfer && cargo build --release --manifest-path /workspace/Cargo.toml && cp /workspace/target/release/sam-sender /usr/local/bin/sam-sender && cp /workspace/target/release/sam-receiver /usr/local/bin/sam-receiver && TEST_ARTIFACT_DIR=/workspace/tmp/test-artifacts/sam-transfer cargo test --manifest-path /workspace/Cargo.toml -p testbed -- test_sam_transfer --nocapture'

# Алиас для всех Rust интеграционных тестов
test-rust:
    @just test

# Очистка временных файлов тестов
clean:
    docker run --rm -v $(pwd):/workspace -w /workspace coursework-overlay:latest rm -rf /workspace/tmp
