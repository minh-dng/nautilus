# syntax=docker/dockerfile:1
FROM ubuntu:24.04

SHELL ["/bin/bash", "-euxo", "pipefail", "-c"]

ARG MISE_VERSION=2026.9.5
ARG AFL_VERSION=4.10c
ARG AFL_SHA256=c9a43894b87502a5f69efdb97dee637c9dd4d2c5dfef1c9d79b9d406adafdb76
ARG USER_ID=1000
ARG GROUP_ID=1000

ENV MISE_DATA_DIR="/mise" \
    MISE_CONFIG_DIR="/mise" \
    MISE_CACHE_DIR="/mise/cache" \
    MISE_INSTALL_PATH="/usr/local/bin/mise" \
    MISE_YES=1 \
    PATH="/mise/shims:${PATH}"

# mise's pipx:basedpyright backend delegates installation to the pipx executable.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      clang \
      curl \
      llvm-dev \
      pipx \
 && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --proto-redir '=https' \
    --fail --show-error --silent --location https://mise.run \
    | MISE_VERSION="${MISE_VERSION}" sh

RUN curl -fsSL "https://github.com/AFLplusplus/AFLplusplus/archive/refs/tags/v${AFL_VERSION}.tar.gz" -o /tmp/afl.tar.gz \
 && echo "${AFL_SHA256}  /tmp/afl.tar.gz" | sha256sum -c - \
 && mkdir /tmp/afl \
 && tar -xzf /tmp/afl.tar.gz -C /tmp/afl --strip-components=1 \
 && make -C /tmp/afl -j"$(nproc)" source-only NO_NYX=1 \
 && test -x /tmp/afl/afl-clang-fast \
 && make -C /tmp/afl install \
 && rm -rf /tmp/afl /tmp/afl.tar.gz

RUN userdel --remove ubuntu \
 && groupadd --gid "${GROUP_ID}" nautilus \
 && useradd --uid "${USER_ID}" --gid "${GROUP_ID}" --create-home --shell /bin/bash nautilus \
 && install -d -o nautilus -g nautilus /mise /workspace /work

WORKDIR /workspace
COPY --chown=nautilus:nautilus . .
USER nautilus

RUN mise trust mise.toml \
 && mise install --locked \
 && ln -s "$(mise where python)" /mise/python \
 && rm -rf "${MISE_CACHE_DIR}"

ENV PYO3_PYTHON=/mise/python/bin/python3 \
    LD_LIBRARY_PATH=/mise/python/lib

RUN test -f "$(python -c 'import sysconfig; print(sysconfig.get_config_h_filename())')" \
 && test -f "$(python -c 'import os, sysconfig; print(os.path.join(sysconfig.get_config_var("LIBDIR"), sysconfig.get_config_var("LDLIBRARY")))')" \
 && mise run target:prepare

CMD ["bash"]
