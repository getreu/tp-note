ARG BASE_IMAGE=rust:1.86-slim-bookworm
WORKDIR /home/project

FROM $BASE_IMAGE as planner

RUN  rustup override set 1.86

# We need theses components
RUN rustup set profile default

# This is the default target
RUN rustup target add x86_64-unknown-linux-gnu

# Add Musl target
RUN rustup target add x86_64-unknown-linux-musl

# Add RaspberryPi target
RUN rustup target add armv7-unknown-linux-gnueabihf
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER /usr/bin/arm-linux-gnueabihf-gcc 
RUN rustup target add aarch64-unknown-linux-gnu
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER /usr/bin/aarch64-linux-gnu-gcc

# Cross compile for Windows
RUN rustup target add x86_64-pc-windows-gnu

# Probably we do not need this
RUN rustup component add rustc cargo rustfmt rust-std clippy rust-docs rust-std # llvm-tools

# Tp-Note needs some libs for crosscompilation.
RUN apt update
RUN apt-get -y install musl-tools crossbuild-essential-armhf crossbuild-essential-arm64

# Cross compile for Windows
RUN apt-get -y install binutils-mingw-w64 mingw-w64

# Pack artifacts in archive.
# Some dependencies need Python, `mc` is for convenience.
RUN apt-get -y install zip python3

# Helper to make deb packages. Use with:
#     cargo deb --target=x86_64-unknown-linux-gnu
RUN cargo install  cargo-deb

# Install `cargo upgrade`.
RUN apt-get -y install libssl-dev
RUN cargo install cargo-edit


# [Cross compiling Windows binaries from Linux](https://jake-shadle.github.io/xwin/)
ENV KEYRINGS /usr/local/share/keyrings

RUN set -eux; \
    mkdir -p $KEYRINGS; \
    apt-get update && apt-get install -y gpg curl; \
    # wine
    curl --fail https://dl.winehq.org/wine-builds/winehq.key | gpg --dearmor > $KEYRINGS/winehq.gpg; \
    echo "deb [signed-by=$KEYRINGS/winehq.gpg] https://dl.winehq.org/wine-builds/debian/ bookworm main" > /etc/apt/sources.list.d/winehq.list;

RUN set -eux; \
    dpkg --add-architecture i386; \
    # Skipping all of the "recommended" cruft reduces total images size by ~300MiB
    apt-get update && apt-get install --no-install-recommends -y \
        # get a recent wine so we can run tests
        winehq-staging \
        # Unpack xwin
        tar; \
    apt-get remove -y --auto-remove; \
    rm -rf /var/lib/apt/lists/*;
    
ENV \
    # wine can be quite spammy with log messages and they're generally uninteresting
    WINEDEBUG="-all" \
    # Use wine to run test executables
    #CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER="wine"
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER="wine"
# Execute tests with
# cargo test --target x86_64-pc-windows-gnu


COPY . .

