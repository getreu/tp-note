# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.145.1/containers/rust/.devcontainer/base.Dockerfile

# FROM mcr.microsoft.com/vscode/devcontainers/rust:0-1
FROM rust:latest

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
     && apt-get -y install --no-install-recommends xorg-dev libxcb-xfixes0-dev libxcb-shape0-dev libgtk-3-dev
