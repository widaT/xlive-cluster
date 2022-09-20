# FROM rust:1.64 as builder
# WORKDIR /usr/src/xlive_cluster
# COPY . .
# #切换docker镜像到国内
# COPY ../docker/config  /usr/local/cargo
# RUN CARGO_HTTP_MULTIPLEXING=false cargo fetch && cargo build --release

# FROM debian:buster-slim
# COPY --from=builder /usr/src/xlive_cluster/targe/xlive-register /usr/local/bin/xlive-register
# EXPOSE 9336/udp
# EXPOSE 3033
# CMD ["xlive-register"]


FROM debian:buster-slim
COPY ./target/release/xlive-register /usr/local/bin/xlive-register
EXPOSE 9336/udp
EXPOSE 3033
CMD ["xlive-register"]