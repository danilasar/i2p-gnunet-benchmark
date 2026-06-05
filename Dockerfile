FROM alt:sisyphus

RUN apt-get update && apt-get install -y \
    gnunet \
    libgnunet \
    i2pd \
    iproute2 \
    procps \
    python3 \
    python3-module-networkx \
    golang \
    curl \
    gcc \
    iputils \
    && apt-get clean

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace


# Build binaries
COPY . /workspace
RUN cargo build --release --manifest-path /workspace/rs/Cargo.toml
RUN cp /workspace/rs/target/release/sam-sender /usr/local/bin/sam-sender
RUN cp /workspace/rs/target/release/sam-receiver /usr/local/bin/sam-receiver
