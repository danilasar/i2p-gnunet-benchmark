FROM alt:sisyphus

RUN apt-get update && apt-get install -y \
    gnunet \
    libgnunet \
    i2pd \
    iproute2 \
    procps \
    python3 \
    python3-module-networkx \
    && apt-get clean

WORKDIR /practice
