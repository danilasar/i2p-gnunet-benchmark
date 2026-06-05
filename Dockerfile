FROM alt:sisyphus

RUN apt-get update && apt-get install -y \
    gnunet \
    libgnunet \
    i2pd \
    iproute2 \
    procps \
    python3 \
    python3-module-networkx \
    curl \
    gcc \
    iputils \
    golang \
    && apt-get clean

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace

# Build binaries
COPY . /workspace
RUN cd /workspace/go-compat && go build -o /usr/local/bin/go-sam3-peer .
RUN cargo build --release --manifest-path /workspace/Cargo.toml
RUN cp /workspace/target/release/sam-sender /usr/local/bin/sam-sender
RUN cp /workspace/target/release/sam-receiver /usr/local/bin/sam-receiver
RUN cp /workspace/target/release/sam-compat /usr/local/bin/sam-compat
