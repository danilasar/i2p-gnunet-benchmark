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

WORKDIR /workspace

# Build binaries
COPY . /workspace
RUN go build -o /usr/local/bin/sam-sender ./cmd/sam-sender/
RUN go build -o /usr/local/bin/sam-receiver ./cmd/sam-receiver/
