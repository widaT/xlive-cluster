FROM rust:1.64 as builder
WORKDIR /usr/src/xlive_cluster
COPY . .
#切换docker镜像到国内
COPY ../docker/config  /usr/local/cargo
RUN CARGO_HTTP_MULTIPLEXING=false cargo fetch && cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/xlive-edge /usr/local/bin/xlive-edge
EXPOSE 3000
EXPOSE 1935
EXPOSE 3032
CMD ["xlive-edge"]
