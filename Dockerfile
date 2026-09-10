# syntax=docker/dockerfile:1
FROM debian:bookworm-slim@sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171

SHELL ["/bin/bash", "-euxo", "pipefail", "-c"]

ARG TARGETARCH
ARG MISE_VERSION=2026.9.3
ARG AFL_VERSION=4.10c
ARG AFL_SHA256=c9a43894b87502a5f69efdb97dee637c9dd4d2c5dfef1c9d79b9d406adafdb76
ARG USER_ID=1000
ARG GROUP_ID=1000

# mise's pipx:basedpyright backend delegates installation to the pipx executable.
RUN sed -i \
      -e 's|http://deb.debian.org/debian-security|http://snapshot.debian.org/archive/debian-security/20260824T000000Z|' \
      -e 's|http://deb.debian.org/debian|http://snapshot.debian.org/archive/debian/20260824T000000Z|' \
      /etc/apt/sources.list.d/debian.sources \
 && printf 'Acquire::Check-Valid-Until "false";\n' >/etc/apt/apt.conf.d/99snapshot \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      clang \
      curl \
      llvm-dev \
      pipx \
 && rm -rf /var/lib/apt/lists/*

RUN case "${TARGETARCH}" in \
      arm64) mise_arch=arm64; mise_sha=d8fa3d3fa2a21979c54c548967325a9db72f2a157996e8c536f62947b25592b7 ;; \
      amd64) mise_arch=x64; mise_sha=981bd9179cc089114a87b491fce2c4007ab05b5445dee7ad08e87e5dffcc154d ;; \
      *) echo >&2 "unsupported architecture: ${TARGETARCH}"; exit 1 ;; \
    esac \
 && curl -fsSL "https://github.com/jdx/mise/releases/download/v${MISE_VERSION}/mise-v${MISE_VERSION}-linux-${mise_arch}" -o /tmp/mise \
 && echo "${mise_sha}  /tmp/mise" | sha256sum -c - \
 && install -m 0755 /tmp/mise /usr/local/bin/mise \
 && rm /tmp/mise

RUN curl -fsSL "https://github.com/AFLplusplus/AFLplusplus/archive/refs/tags/v${AFL_VERSION}.tar.gz" -o /tmp/afl.tar.gz \
 && echo "${AFL_SHA256}  /tmp/afl.tar.gz" | sha256sum -c - \
 && mkdir /tmp/afl \
 && tar -xzf /tmp/afl.tar.gz -C /tmp/afl --strip-components=1 \
 && make -C /tmp/afl -j"$(nproc)" source-only NO_NYX=1 \
 && test -x /tmp/afl/afl-clang-fast \
 && make -C /tmp/afl install \
 && rm -rf /tmp/afl /tmp/afl.tar.gz

RUN groupadd --gid "${GROUP_ID}" nautilus \
 && useradd --uid "${USER_ID}" --gid "${GROUP_ID}" --create-home --shell /bin/bash nautilus \
 && install -d -o nautilus -g nautilus /opt/mise /workspace /work

ENV MISE_DATA_DIR=/opt/mise \
    PATH=/opt/mise/shims:${PATH}

WORKDIR /workspace
COPY --chown=nautilus:nautilus . .
USER nautilus

RUN mise trust mise.toml \
 && mise install --locked \
 && ln -s "$(mise where python)" /opt/mise/python

ENV PYO3_PYTHON=/opt/mise/python/bin/python3 \
    LD_LIBRARY_PATH=/opt/mise/python/lib

RUN test -f "$(python -c 'import sysconfig; print(sysconfig.get_config_h_filename())')" \
 && test -f "$(python -c 'import os, sysconfig; print(os.path.join(sysconfig.get_config_var("LIBDIR"), sysconfig.get_config_var("LDLIBRARY")))')" \
 && mise run target:prepare

CMD ["bash"]
