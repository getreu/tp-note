ARG BASE_IMAGE=rust:slim-buster

FROM $BASE_IMAGE as planner
WORKDIR /home/dockeruser/project

# Add Rust format tool.
RUN rustup component add rustfmt
### use with
# RUN cargo fmt

# Add Rust clippy.
RUN rustup component add clippy
### use with
# RUN cargo clippy

# Add Musl target
RUN rustup target add x86_64-unknown-linux-musl
### use with
# RUN PKG_CONFIG_ALLOW_CROSS=1 cargo build --target x86_64-unknown-linux-musl --release

# Add Musl target
RUN rustup target add x86_64-unknown-linux-musl
### use with
# RUN PKG_CONFIG_ALLOW_CROSS=1 cargo build --target x86_64-unknown-linux-musl --release

# Cross compile for Windows
RUN rustup target add x86_64-pc-windows-gnu
### use with
# RUN cargo build --target x86_64-pc-windows-gnu


RUN apt-get update

# Pack artifacts in archive.
# Some dependencies need Python, `mc` is for convenience.
RUN apt-get -y install zip python3 mc

# Tp-Note needs GTK dev for the `message-box` feature.
RUN apt-get -y install --no-install-recommends xorg-dev \
        libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev

# Cross compile for Windows
RUN apt-get -y install binutils-mingw-w64 mingw-w64
### use with
# RUN cargo build --target x86_64-pc-windows-gnu

# Helper to make deb packages.
RUN cargo install cargo-deb
### use with
# RUN cargo deb --target=x86_64-unknown-linux-gnu

# Add a tool to upgrade dependencies.
RUN apt-get -y install libssl-dev
RUN cargo install cargo-edit
### use with
#RUN cargo upgrade

COPY . .

