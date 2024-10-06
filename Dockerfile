# 构建阶段
FROM rust:1.78.0-slim-buster as chef
WORKDIR /app
RUN apt update && apt install lld clang libssl-dev pkg-config -y && cargo install cargo-chef --locked

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# 构建我们的项目依赖项，而不是我们的应用程序！
RUN cargo chef cook --release --recipe-path recipe.json
# 到目前为止，如果我们的依赖树没有变化，
# 所有层都应该被缓存。
COPY . .
ENV SQLX_OFFLINE true
RUN cargo build --release

# 运行时阶段
FROM debian:bullseye-slim AS runtime
WORKDIR /app

COPY --from=builder /app/target/release/zero2prod zero2prod
COPY config.yaml .

ENTRYPOINT [ "./zero2prod" ]

