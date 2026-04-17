FROM node:22-bookworm

ENV http_proxy=http://host.docker.internal:7890
ENV https_proxy=http://host.docker.internal:7890
ENV HTTP_PROXY=http://host.docker.internal:7890
ENV HTTPS_PROXY=http://host.docker.internal:7890
ENV ALL_PROXY=socks5://host.docker.internal:7890
ENV no_proxy=localhost,127.0.0.1

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    zip \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
ENV CARGO_HOME="/root/.cargo"

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

RUN npm install -g pnpm@10
RUN pnpm config set store-dir /root/.local/share/pnpm/store

RUN git config --global user.email "hpp2334@outlook.com" && \
    git config --global user.name "hpp2334"
