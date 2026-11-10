# Clean Ubuntu target for Carrova end-to-end tests. The carrova binary and
# recipes are bind-mounted at run time (no rebuild per code change).
FROM ubuntu:24.04
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      git openssh-client zsh gnupg ca-certificates procps \
 && rm -rf /var/lib/apt/lists/* \
 && useradd -m -s /bin/bash dev
USER dev
WORKDIR /home/dev
ENV CARROVA_RECIPES=/carrova/recipes PATH=/carrova/bin:/usr/local/bin:/usr/bin:/bin
