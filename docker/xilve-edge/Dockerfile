# FROM rust:1.64 as builder
# WORKDIR /usr/src/xlive_cluster
# COPY . .
# #切换docker镜像到国内
# COPY ../docker/config  /usr/local/cargo
# RUN CARGO_HTTP_MULTIPLEXING=false cargo fetch && cargo build --release

# FROM debian:buster-slim
# COPY --from=builder /usr/local/cargo/bin/xlive-edge /usr/local/bin/xlive-edge
# EXPOSE 3000
# EXPOSE 1935
# EXPOSE 3032
# CMD ["xlive-edge"]

FROM debian:buster-slim
COPY ./temp/xlive-edge /usr/local/bin/xlive-edge
EXPOSE 3000
EXPOSE 1935
EXPOSE 3032
CMD ["xlive-edge"]