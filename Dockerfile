ARG BUILD_RUST_TAG=latest
ARG FINAL_RUST_TAG=latest

# This stage sets up everything we need to cross-compile Rust programs in other
# stages.
FROM --platform=$BUILDPLATFORM "docker.io/library/rust:$BUILD_RUST_TAG" AS build

# Install platform-agnostic dependencies.
#
# Check for the existence of APK/APT to determine how to install dependencies.
# hadolint ignore=DL3008,DL3018 # We don't want to pin package versions.
RUN --mount=type=cache,target=/var/cache/apk \
    --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    ( which apk && apk add clang ) || \
    ( \
        which apt-get && \
        apt-get update && \
        apt-get --no-install-recommends --assume-yes install clang \
    )

# Install cross-compilation helper scripts. See
# https://github.com/tonistiigi/xx#rust.
# hadolint ignore=DL3067 # We really do want to copy everything from this image.
COPY --from=docker.io/tonistiigi/xx:latest / /

# Everything from this point onwards is specific to the target platform.
ARG TARGETPLATFORM

# Install platform-specific dependencies.
#
# Check for the existence of APK/APT to determine how to install dependencies.
# hadolint ignore=DL3008,DL3018 # We don't want to pin package versions.
RUN --mount=type=cache,target=/var/cache/apk \
    --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    ( which apk && xx-apk add xx-c-essentials ) || \
    ( \
        which apt-get && \
        xx-apt-get update && \
        xx-apt-get --no-install-recommends --assume-yes install \
            xx-c-essentials \
    )

# Use the prepared build stage to build `cargo-mutants`.
FROM build AS build-mutants

# Build cargo-mutants.
#
# Cache `/var/cache/cargo` (dependencies are downloaded here) and `/tmp/target`
# (compilation artifacts are generated here). These downloads/artifacts can be
# cached between invocations of `docker build`.
#
# Use `xx-verify` to confirm that the installed binary was correctly
# cross-compiled for the target architecture.
RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=mutants_attrs,target=mutants_attrs \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/tmp/target \
    --mount=type=cache,target=/var/cache/cargo \
    CARGO_HOME=/var/cache/cargo \
    xx-cargo install \
        --path . \
        --root / \
        --target-dir /tmp/target \
        --locked && \
    xx-verify /bin/cargo-mutants

# Create the final image by adding `cargo-mutants` to the Rust image.
FROM "docker.io/library/rust:$FINAL_RUST_TAG" AS final

COPY --from=build-mutants /bin/cargo-mutants /usr/local/cargo/bin/

WORKDIR /app

ENTRYPOINT [ "cargo", "mutants" ]
