ARG BASE_IMAGE=rust:slim-buster

FROM $BASE_IMAGE as planner
WORKDIR /home/dockeruser/project

# Add Rust format tool.
RUN rustup component add rustfmt
### use with
# cargo fmt

# Add Rust clippy.
RUN rustup component add clippy
### use with
#  cargo clippy


# Add Musl target
RUN rustup target add x86_64-unknown-linux-musl

# Add RaspberryPi target
RUN rustup target add armv7-unknown-linux-gnueabihf
RUN rustup target add aarch64-unknown-linux-gnu

# Cross compile for Windows
RUN rustup target add x86_64-pc-windows-gnu

# Tp-Note needs some libs for crosscompilation.
RUN apt update
RUN apt-get -y install musl-tools crossbuild-essential-armhf crossbuild-essential-arm64

# Cross compile for Windows
RUN apt-get -y install binutils-mingw-w64 mingw-w64

# Pack artifacts in archive.
# Some dependencies need Python, `mc` is for convenience.
RUN apt-get -y install zip python3 mc

# Helper to make deb packages.
RUN cargo install cargo-deb
### use with
# cargo deb --target=x86_64-unknown-linux-gnu

# Add a tool to upgrade dependencies.
RUN apt-get -y install libssl-dev pkg-config
RUN cargo install cargo-edit
### use with
# cargo upgrade

COPY . .

