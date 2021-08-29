ARG BASE_IMAGE=rust
ARG CHEF_TAG
FROM $BASE_IMAGE

# Install musl-dev on Alpine to avoid error "ld: cannot find crti.o: No such file or directory"
RUN ((cat /etc/os-release | grep ID | grep alpine) && apk add --no-cache musl-dev || true) \
    && cargo install cargo-chef --version $CHEF_TAG \
    && rm -rf $CARGO_HOME/registry/
