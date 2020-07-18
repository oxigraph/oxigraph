FROM rust:1-buster as builder

RUN apt-get update && apt-get install clang -y
COPY . /oxigraph
RUN cd /oxigraph/wikibase && cargo build --release

FROM debian:buster-slim

RUN apt-get update && apt-get install ca-certificates -y && rm -rf /var/lib/apt/lists/*
COPY --from=builder /oxigraph/target/release/oxigraph_wikibase /usr/local/bin/oxigraph_wikibase

ENTRYPOINT [ "/usr/local/bin/oxigraph_wikibase" ]
