FROM lukemathwalker/cargo-chef:latest-rust-1.74.0 as chef

WORKDIR /app

# install requirements for linking configuration
RUN apt update && apt install lld clang -y

FROM chef as planner
# copy all files from working directory to docker image
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# build project deps
RUN cargo chef cook --release --recipe-path recipe.json
# following code only runs if dependency tree has changed
COPY . .
RUN cargo build --release --bin chess-bot

FROM debian:bookworm-slim as runtime
WORKDIR /app
# install ca-certificates (to verify TLS certs for https connections)
RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	# clean up
	&& apt-get autoremove -y \
	&& apt-get clean -y \
	&& rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/chess-bot chess-bot
COPY --from=builder /app/.env .

# execute on `docker run`
ENTRYPOINT [ "./chess-bot" ]
