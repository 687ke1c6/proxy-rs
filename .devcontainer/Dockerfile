FROM rust:trixie

# RUN apt-get update && apt-get install -y --no-install-recommends \
#     build-essential \
#     ca-certificates \
#     curl \
#     git \
#     iproute2 \
#     && rm -rf /var/lib/apt/lists/* \
#     && apt-get autoremove -y

ARG UID=1000
ARG GID=1000

RUN groupadd -g ${GID} rust && \
    useradd -m -u ${UID} -g ${GID} -s /bin/bash rust

USER rust