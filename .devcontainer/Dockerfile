# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.241.1/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="buster"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
        python3 \
        python3-venv \
        python-is-python3 \
        libclang-dev

ENV VIRTUAL_ENV=/opt/venv
RUN python -m venv $VIRTUAL_ENV
ENV PATH="$VIRTUAL_ENV/bin:$PATH"
RUN pip install --no-cache-dir \
    "maturin>=0.9,<0.10" \
    sphinx \
    sphinx-autobuild \
    furo

# Change owner to the devcontainer user
RUN chown -R 1000:1000 $VIRTUAL_ENV
