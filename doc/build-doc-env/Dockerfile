ARG BASE_IMAGE=rust:1.54-slim-buster

FROM $BASE_IMAGE as planner
WORKDIR /home/dockeruser/project

RUN apt-get update

# `mc` is for convenience.
RUN apt-get -y install mc

# Build environment for documentation
# HTML: see `docbook52htm`
RUN apt-get -y install pandoc docbook-xsl-ns xsltproc
# PDF: see `docbook52pdf`
RUN apt-get -y install pandoc docbook5-xml docbook-xsl-ns xsltproc fop xmlto libxml2-utils xmlstarlet

COPY . .

