FROM rust:1.44 as builder

RUN apt-get update && \
  apt-get install clang -y && \
  git clone https://github.com/oxigraph/oxigraph --depth 1 && \
  cd oxigraph && \
  cargo build --release

FROM debian:buster-slim

COPY --from=builder /oxigraph/target/release/oxigraph_server /usr/local/bin/oxigraph_server

ENTRYPOINT [ "/usr/local/bin/oxigraph_server" ]
