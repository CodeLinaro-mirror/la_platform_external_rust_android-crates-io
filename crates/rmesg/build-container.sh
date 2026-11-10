#!/bin/sh


IMAGE="ghcr.io/archisgore/rust-dev-env"
docker run -v cargo-cache:/root/.cargo/registry -v $PWD:/volume --rm -it --privileged $IMAGE
