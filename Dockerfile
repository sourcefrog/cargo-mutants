# syntax=docker/dockerfile:1
FROM rust:latest AS build
RUN apt update
RUN apt install -y s3cmd tar zstd awscli clang build-essential
RUN adduser --disabled-password --gecos "mutants" mutants

USER mutants
WORKDIR /home/mutants

RUN cargo install --locked cargo-nextest
RUN --mount=type=bind,source=.,target=/src,rw cargo install --locked --path=/src

# TODO: Copy everything to a final image that doesn't need the build artifacts?

# FROM rust:latest AS final
# COPY --from=build /build/release/cargo-mutants /usr/local/bin/
