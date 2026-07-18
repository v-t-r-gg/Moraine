# Build stage
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/moraine-core crates/moraine-core
COPY crates/moraine-cli crates/moraine-cli
COPY crates/moraine-server crates/moraine-server
COPY src-tauri src-tauri
RUN cargo build --release -p moraine-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/moraine-server /usr/local/bin/moraine-server
ENV MORAINE_BIND=0.0.0.0:3099
EXPOSE 3099
USER nobody
ENTRYPOINT ["moraine-server"]
