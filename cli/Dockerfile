FROM --platform=$BUILDPLATFORM rust:1-bookworm AS builder
ARG BUILDARCH TARGETARCH
RUN apt-get update && \
    apt-get install -y libclang-dev clang && \
    if [ "$BUILDARCH" != "$TARGETARCH" ] && [ "$TARGETARCH" = "arm64" ] ; \
    then \
        apt-get install -y g++-aarch64-linux-gnu && \
        rustup target add aarch64-unknown-linux-gnu ; \
    fi
COPY . /oxigraph
WORKDIR /oxigraph/cli
RUN if [ "$BUILDARCH" != "$TARGETARCH" ] && [ "$TARGETARCH" = "arm64" ] ; \
    then \
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc && \
        export BINDGEN_EXTRA_CLANG_ARGS="--sysroot /usr/aarch64-linux-gnu" && \
        cargo build --release --target aarch64-unknown-linux-gnu --no-default-features --features rustls-webpki,geosparql,rdf-12 && \
        mv /oxigraph/target/aarch64-unknown-linux-gnu/release/oxigraph /oxigraph/target/release/oxigraph ; \
    else \
        cargo build --release --no-default-features --features rustls-webpki,geosparql,rdf-12 ; \
    fi

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /oxigraph/target/release/oxigraph /usr/local/bin/oxigraph
ENTRYPOINT [ "/usr/local/bin/oxigraph" ]
CMD [ "serve", "--location", "/data", "--bind", "0.0.0.0:7878" ]
