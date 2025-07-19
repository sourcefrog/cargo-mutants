# syntax=docker/dockerfile:1
FROM rust:latest AS build
RUN --mount=type=bind,source=.,target=/src,rw cargo install --locked --path=/src --target-dir=/build

FROM rust:latest AS final
COPY --from=build /build/release/cargo-mutants /usr/local/bin/
