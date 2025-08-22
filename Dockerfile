# syntax=docker/dockerfile:1
FROM rust:latest AS base
RUN apt update && apt dist-upgrade -y && apt install -y s3cmd tar zstd awscli clang build-essential
RUN adduser --uid 1000 --disabled-password --gecos "mutants" mutants

FROM base AS build
WORKDIR /home/mutants
USER mutants
COPY --chown=mutants:mutants . mutants-src
RUN --mount=type=cache,target=mutants-src/target,uid=1000 \
    --mount=type=cache,target=/home/mutants/.cargo,uid=1000 \
    cargo install -v --locked --path=mutants-src --root=buildroot
RUN pwd && ls -la . buildroot
RUN --mount=type=cache,target=/home/mutants/.cargo,uid=1000 \
    cargo install --locked cargo-nextest --root=buildroot

FROM base AS final
USER mutants
COPY --from=build /home/mutants/buildroot/bin/* /usr/local/bin/
