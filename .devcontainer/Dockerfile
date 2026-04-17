FROM rust:trixie

ARG UID=1000
ARG GID=1000

RUN groupadd -g ${GID} rust && \
    useradd -m -u ${UID} -g ${GID} -s /bin/bash rust

USER rust