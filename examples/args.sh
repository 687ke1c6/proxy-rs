#!/usr/bin/env bash
# Example argument parsing: a case per flag, each of which shifts past the
# flag and prints its value.
#
#   examples/args.sh -n myserver

case "$1" in
    whoami)
        docker run -it -v ./.proxy-rs-docker:/home/rust/.proxy-rs -p 3001:3001 proxy-rs proxy-rs client whoami
        ;;
    "")
        docker run -it -v ./.proxy-rs-docker:/home/rust/.proxy-rs -p 3001:3001 proxy-rs proxy-rs client socks5 --listen 0.0.0.0:3001
        ;;
    *)
        echo "unknown argument: $1"
        ;;
esac
