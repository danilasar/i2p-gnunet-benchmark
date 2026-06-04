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

RUN apt-get install -y iputils

# Install Rust via rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /practice

# Build rust stubs
COPY rs /practice/rs
RUN cd /practice/rs && cargo build
