##
## Shurly
##

# Base builder image
FROM rust:1.70-slim as builder

# Very nice
WORKDIR /usr/src/shurly

# Add the entire source
COPY . .

# We setup a SQLx cache file of our schema to support building without a database connection
ENV SQLX_OFFLINE true

# We be building!
RUN --mount=type=cache,target=/usr/src/shurly/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release; \
    # move binary out of cached directory, so the runtime can copy it
    objcopy --compress-debug-sections target/release/shurly ./shurly

# Lean, mean, image machine
FROM gcr.io/distroless/cc as runtime

# It's us
LABEL org.opencontainers.image.source https://github.com/workplacebuddy/shurly

# Just the Shurly binary
COPY --from=builder /usr/src/shurly/shurly /

# Run, Shurly, run!
ENTRYPOINT ["./shurly"]
