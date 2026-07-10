# Use `docker build --build-arg RUST_IMAGE_TAG=whatever .` to specify a
# different variant of the Rust image when building this image.
ARG RUST_IMAGE_TAG=latest

FROM "docker.io/library/rust:$RUST_IMAGE_TAG"

# Use `docker build --build-arg CARGO_MUTANTS_VERSION=whatever .` to specify the
# version cargo-mutants to install in this image.
ARG CARGO_MUTANTS_VERSION

# Install cargo-mutants.
#
# - Cache `/usr/local/cargo/git/db` and `/usr/local/cargo/registry`. Cargo
#   downloads dependencies here. These downloads should not appear in the final
#   image and can be cached between invocations of `docker build`.
# - Cache `/cargo-mutants`. Cargo writes compilation artifacts here (see
#   `--target-dir` argument). These artifacts should not appear in the final
#   image and can be cached between invocations of `docker build`.
# - Mask `/tmp`. We depend on `rustix`, which writes a file here when its build
#   script runs. This file should not appear in the final image. See
#   https://github.com/bytecodealliance/rustix/blob/035acdc416d9465abc937bd8cd6a0031afc170aa/build.rs#L248.
RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/cargo-mutants \
    --mount=type=tmpfs,target=/tmp \
    cargo install \
        --locked \
        --target-dir /cargo-mutants \
        "cargo-mutants@$CARGO_MUTANTS_VERSION"

WORKDIR /app

ENTRYPOINT [ "cargo", "mutants" ]
