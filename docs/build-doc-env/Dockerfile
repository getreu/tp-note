ARG BASE_IMAGE=alpine:latest

FROM $BASE_IMAGE as planner
WORKDIR /home/project

# RUN apk add bash pandoc weasyprint
RUN apk --update --upgrade --no-cache add bash pandoc weasyprint pango
#RUN apk --no-cache add py3-pip gcc musl-dev python3-dev pango zlib-dev jpeg-dev openjpeg-dev g++ libffi-dev

# Additional font 
RUN apk --update --upgrade --no-cache add fontconfig ttf-freefont font-noto terminus-font \ 
     && fc-cache -f \ 
     && fc-list | sort 

COPY . .

