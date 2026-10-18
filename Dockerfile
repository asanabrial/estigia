FROM rust:bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
COPY --from=build /src/target/release/estigia /usr/local/bin/estigia
ENTRYPOINT ["estigia", "mcp"]
