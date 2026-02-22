# Copyright 2025 Asher Buk
#
# SPDX-License-Identifier: Apache-2.0
#
# https://github.com/AshBuk/Shaya

# Shaya Build Environment
# Pure Rust + GTK4 + libadwaita

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    curl \
    wget \
    git \
    file \
    libssl-dev \
    pkg-config \
    meson \
    ninja-build \
    # GTK4 + libadwaita (Ubuntu 24.04 has libadwaita 1.5)
    libgtk-4-dev \
    libadwaita-1-dev \
    libsoup-3.0-dev \
    libgraphene-1.0-dev \
    # Wayland
    libwayland-dev \
    wayland-protocols \
    libgirepository1.0-dev \
    # Audio
    libasound2-dev \
    libpulse-dev \
    # Utilities
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Build gtk4-layer-shell from source
RUN git clone --depth 1 --branch v1.3.0 https://github.com/wmww/gtk4-layer-shell.git \
    && cd gtk4-layer-shell \
    && meson setup build -Dvapi=false -Dintrospection=false \
    && ninja -C build \
    && ninja -C build install \
    && ldconfig \
    && cd .. \
    && rm -rf gtk4-layer-shell

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && . $HOME/.cargo/env \
    && rustup default stable

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

RUN rustc --version && cargo --version && pkg-config --modversion libadwaita-1

CMD ["/bin/bash"]
