# FROM rust:1.63 as builder
# WORKDIR /usr/src/xlive_cluster
# COPY . .
# #切换docker镜像到国内
# COPY ./docker/cargo_config/config  /usr/local/cargo
# RUN CARGO_HTTP_MULTIPLEXING=false cargo fetch && cargo build --release

# FROM debian:buster-slim
# COPY --from=builder /usr/local/cargo/bin/xlive-origin /usr/local/bin/xlive-origin
# EXPOSE 9878
# EXPOSE 3032

#CMD ["xlive-origin"]


FROM debian:buster-slim
COPY ./temp/xlive-origin /usr/local/bin/xlive-origin
EXPOSE 9878
EXPOSE 3032
CMD ["xlive-origin"]