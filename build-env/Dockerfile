ARG BASE_IMAGE=rust:1.79-slim-buster
WORKDIR /home/project

FROM $BASE_IMAGE as planner

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
RUN apt-get -y install zip python3

# Helper to make deb packages.
RUN cargo install cargo-deb
### use with
# cargo deb --target=x86_64-unknown-linux-gnu


COPY . .

