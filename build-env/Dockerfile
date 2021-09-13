ARG BASE_IMAGE=rust:1.54-slim-buster

FROM $BASE_IMAGE as planner
WORKDIR /home/getreu/project

RUN apt-get update

# Some dependencies need Python, `mc` is for convenience.
RUN apt-get -y install python3 mc

# Tp-Note needs GTK dev for the `message-box` feature.
RUN apt-get -y install --no-install-recommends xorg-dev \
        libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev

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

# Build environment for documentation
# HTML: see `docbook52htm`
RUN apt-get -y install pandoc docbook-xsl-ns xsltproc
# PDF: see `docbook52pdf`
RUN apt-get -y install pandoc docbook5-xml docbook-xsl-ns xsltproc fop xmlto libxml2-utils xmlstarlet

COPY . .

