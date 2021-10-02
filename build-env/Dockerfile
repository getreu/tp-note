ARG BASE_IMAGE=rust:1.54-slim-buster

FROM $BASE_IMAGE as planner
WORKDIR /home/dockeruser/project

RUN apt-get update

# Some dependencies need Python, `mc` is for convenience.
RUN apt-get -y install python3 mc

# Tp-Note needs GTK dev for the `message-box` feature.
RUN apt-get -y install --no-install-recommends xorg-dev \
        libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev

RUN apt-get -y install binutils-mingw-w64

# Add Rust format tool.
RUN rustup component add rustfmt
### use with
# RUN cargo fmt

# Add Rust clippy.
RUN rustup component add clippy
### use with
# RUN cargo clippy

# Add a tool to upgrade dependencies.
RUN apt-get -y install libssl-dev
RUN cargo install cargo-edit
### use with
# RUN cargo upgrade

# Add Musl target
RUN rustup target add x86_64-unknown-linux-musl
### use with
# RUN PKG_CONFIG_ALLOW_CROSS=1 cargo build --target x86_64-unknown-linux-musl --release

# Helper to make deb packages.
RUN cargo install cargo-deb
### use with
# RUN cargo deb --target=x86_64-unknown-linux-gnu

# Cross compile for Windows
RUN apt-get -y install mingw-w64
RUN rustup target add x86_64-pc-windows-gnu
### use with
# RUN cargo build --target x86_64-pc-windows-gnu

# Pack artifacts in archive.
RUN apt-get -y install zip

COPY . .

