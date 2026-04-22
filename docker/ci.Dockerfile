FROM node:22-bookworm

ENV http_proxy=http://host.docker.internal:7890
ENV https_proxy=http://host.docker.internal:7890
ENV HTTP_PROXY=http://host.docker.internal:7890
ENV HTTPS_PROXY=http://host.docker.internal:7890
ENV ALL_PROXY=socks5://host.docker.internal:7890
ENV no_proxy=localhost,127.0.0.1

RUN apt-get update && apt-get install -y build-essential && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y zip && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y libx11-dev && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y libxcb1-dev && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y xvfb && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
ENV CARGO_HOME="/root/.cargo"

RUN curl -L https://github.com/mozilla/sccache/releases/download/v0.14.0/sccache-v0.14.0-x86_64-unknown-linux-musl.tar.gz | tar xz --strip-components=1 -C /root/.cargo/bin sccache-v0.14.0-x86_64-unknown-linux-musl/sccache && chmod +x /root/.cargo/bin/sccache

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

RUN rustup target add wasm32-unknown-unknown

RUN npm install -g pnpm@10
RUN pnpm config set store-dir /root/.local/share/pnpm/store

COPY assets/prewarm.zip /tmp/prewarm.zip
RUN unzip /tmp/prewarm.zip -d /tmp/prewarm && \
    cd /tmp/prewarm && \
    node scripts/prewarm.cjs && \
    rm -rf /tmp/prewarm /tmp/prewarm.zip

RUN git config --global user.email "hpp2334@outlook.com" && \
    git config --global user.name "hpp2334"
