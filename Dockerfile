##
## Shurly
##
#
# Inspirations:
# - Cargo chef to cache the dependencies in a Docker layer
#   https://www.lpalmieri.com/posts/fast-rust-docker-builds/
#

# Base builder image
FROM rust:1.62 as chef
RUN cargo install cargo-chef
WORKDIR /usr/src/shurly

# Setup the chef planner
FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path chef-recipe.json

# Build Shurly
FROM chef as builder

# Change Shurly features, "postgres" or "memory"
ARG SHURLY_FEATURES="memory"
ENV SHURLY_FEATURES=$SHURLY_FEATURES

# Cook the chef recipe
COPY --from=planner /usr/src/shurly/chef-recipe.json chef-recipe.json
RUN cargo chef cook --features=$SHURLY_FEATURES --release --recipe-path chef-recipe.json

COPY . .

# We setup a SQLx cache file of our schema to support building without a database connection
ENV SQLX_OFFLINE true

# We be building!
RUN cargo build --features=$SHURLY_FEATURES --release

# Lean, mean, image machine
FROM debian:buster-slim as runtime

# Just the Shurly binary
COPY --from=builder /usr/src/shurly/target/release/shurly /usr/local/bin/shurly

# Run, Shurly, run!
ENTRYPOINT ["shurly"]
