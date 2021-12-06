FROM rust:1-buster as builder

RUN apt-get update && \
    apt-get install -y libclang-dev clang

COPY . /oxigraph
WORKDIR /oxigraph/server 
RUN cargo build --release


FROM debian:buster-slim
LABEL org.opencontainers.image.source="https://github.com/oxigraph/oxigraph"

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /oxigraph/target/release/oxigraph_server /usr/local/bin/oxigraph_server

ENTRYPOINT [ "/usr/local/bin/oxigraph_server" ]
CMD [ "--location", "/data", "serve", "--bind", "0.0.0.0:7878" ]
