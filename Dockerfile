FROM rust:alpine AS builder

# musl-dev/gcc: ring's C code needs them to compile and link.
RUN apk add --no-cache gcc musl-dev

WORKDIR /app
COPY . .

RUN cargo install --path .

FROM alpine AS runner

ARG UID=1000
ARG GID=1000

# rsync: needed by `server -r` and `client sync-rsh`.
RUN apk add --no-cache ca-certificates rsync

RUN addgroup -g ${GID} rust && \
    adduser -D -u ${UID} -G rust rust

COPY --from=builder /usr/local/cargo/bin/proxy-rs /usr/local/bin/proxy-rs

USER rust
WORKDIR /home/rust

CMD ["proxy-rs", "server", "-t", "'*'" "--allow-any"]
