ARG BASE_IMAGE=rust
FROM $BASE_IMAGE
ARG CHEF_TAG

# Install musl-dev on Alpine to avoid error "ld: cannot find crti.o: No such file or directory"
RUN ((cat /etc/os-release | grep ID | grep alpine) && apk add --no-cache musl-dev || true) \
    && CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install cargo-chef --locked --version $CHEF_TAG \
    && rm -rf $CARGO_HOME/registry/
