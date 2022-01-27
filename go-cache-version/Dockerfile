FROM golang:1.17.6-alpine

RUN apk add --update upx bash

COPY build.sh go.mod go.sum skipIfTreeOkInPast.go /src/

WORKDIR /src/

RUN ./build.sh
