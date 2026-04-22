FROM rust:trixie

ARG UID=1000
ARG GID=1000

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    git \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -g ${GID} rust && \
    useradd -m -u ${UID} -g ${GID} -s /bin/bash rust

RUN mkdir /app && chown rust:rust /app

USER rust

WORKDIR /app
COPY --chown=rust:rust . .

RUN cargo install --path .

CMD ["proxy-rs"]