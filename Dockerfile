FROM rust:1.71

WORKDIR /usr/src/tellme
COPY . .

RUN cargo install --path .

CMD tellme

EXPOSE 8080