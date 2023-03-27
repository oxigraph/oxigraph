FROM rust:1-bullseye as builder
RUN apt-get update && \
    apt-get install -y libclang-dev clang
COPY . /oxigraph
WORKDIR /oxigraph/server 
RUN cargo build --release

FROM gcr.io/distroless/cc-debian11
COPY --from=builder /oxigraph/target/release/oxigraph_server /usr/local/bin/oxigraph_server
ENTRYPOINT [ "/usr/local/bin/oxigraph_server" ]
CMD [ "serve", "--location", "/data", "--bind", "0.0.0.0:7878" ]
