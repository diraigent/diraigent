# Lightweight CI image for building and pushing container images.
# Pre-installs Docker CLI + Buildx + Node.js so deploy workflows
# don't have to apt-get install them from scratch every run.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl gnupg git xz-utils libatomic1 \
    && install -m 0755 -d /etc/apt/keyrings \
    && curl -fsSL https://download.docker.com/linux/debian/gpg \
        | gpg --dearmor -o /etc/apt/keyrings/docker.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
        https://download.docker.com/linux/debian bookworm stable" \
        > /etc/apt/sources.list.d/docker.list \
    && apt-get update && apt-get install -y --no-install-recommends \
        docker-ce-cli docker-buildx-plugin \
    && curl -fsSL https://nodejs.org/dist/v25.6.1/node-v25.6.1-linux-x64.tar.xz \
        | tar xJ -C /usr/local --strip-components=1 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

LABEL org.opencontainers.image.title="diraigent-ci-deploy" \
      org.opencontainers.image.description="CI image with Docker CLI, Buildx, and Node.js for deploy workflows"
