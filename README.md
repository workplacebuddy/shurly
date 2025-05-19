# Shurly

> Shurly, this is a URL shortener with API management

## Features

-   Management of destinations through a REST'ish API
-   Permanent/temporary redirects; permanent redirect can not be changed after
    creation
-   Add notes to destinations to keep track of where destinations are being used
-   Track all hits on destinations, with user agent and ip addres (if possible)
-   Audit log for all creative/destructive management actions

## Quick usage

```sh
# Create destination
curl -v -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer tokentokentoken' \
    -d '{ "slug": "the-one", "url": "https://www.example.com/" }' \
    http://localhost:7000/api/destinations

# The redirect
curl -v http://localhost:7000/the-one

# Response:
# < HTTP/2 307
# < Location: https://www.example.com/
```

## Getting started

### Setup `PostgreSQL` database

> An extra requirement is needed to actually run Shurly with a database, that is
> an actual database. This can be setup separately, or the [Docker Compose] setup
> can be used, which will run a PostgreSQL server container. In the end, Shurly
> needs a a valid `DATABASE_URL` to be available.

There are a couple of ways to run this, all depending on your preference:

Shurly can be directly installed with `cargo install shurly`, this will place a
`shurly` binary in your path (if the Cargo installation directly is in your
`PATH`).

Building it yourself is also possible, of course, there are a couple more
options. To start, you need to clone the repo.

```sh
git clone git@github.com:workplacebuddy/shurley.git
```

Then it's only a `cargo run` away. If [Docker] has your preference, then
building it is also possible.

```sh
docker build --tag shurly .
```

Running it is the same as for all Docker container.

```sh
docker run --rm --interactive --tty --publish 7000:7000 shurly .

# for short
docker run --rm -it -p 7000:7000 shurly .
```

## Usage

Shurly has a very simple REST'ish interface to management the destinations and
its properties.

### The root

Everything that is not matched by an API route will be handled by the root as a
fallback, this root will look up a destination based on its path and will
either redirect to that destination or show a 404.

Redirects are done based on the `isPermanent` property of a destination;
Permanent redirects are done with the 308 (Permanent Redirect) status code and
the temporary redirect uses the 307 (Temporary Redirect) redirect. Both will
set the `Location` header to the associated URL.

### Management

Only authorized users can manage destinations and need to get a token to access
the other API endpoints.

```sh
curl -v -H 'Content-Type: application/json' \
    -d '{ "username": "admin", "password": "verysecret" }' \
    http://localhost:7000/api/users/token

# < { "data": { "type": "Bearer", "access_token": "some token" } }
```

To create destinations the `/api/destinations` URL can be posted to with a
payload to describe what needs to happen when.

```sh
curl -v -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer tokentokentoken' \
    -d '{ "slug": "some-easy-name", "url": "https://www.example.com/" }' \
    http://localhost:7000/api/destinations

# < { "data": { "id": "<uuid>", "slug": "some-easy-name" ... } }
```

Optionally you can send the `isPermanent` property, to indicate what kind of
redirect should be used. Permanent redirects can not be changed after they are
created.

Updating a destination happens in the same fashion.

```sh
curl -v XPATCH -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer tokentokentoken' \
    -d '{ "url": "https://www.example.com/", "isPermanent": true }' \
    http://localhost:7000/api/destinations/<uuid>

# < { "data": { "id": "<uuid>", "slug": "some-easy-name" ... } }
```

The `slug` can not be changed after creation. Changing the `isPermanent` flag
_to_ `true` is possible, not the other way around. When `isPermanent` is
`true`, updating the `url` will fail.

To remove the destination, a `DELETE` endpoint is available.

```sh
curl -v XDELETE \
    -H 'Authorization: Bearer tokentokentoken' \
    http://localhost:7000/api/destinations/<uuid>
```

This will soft-delete the destination; creating a new destination with the same
slug is not possible: creativity is key.

There are a bunch more interactions available, but this should get you going.


```sh
# Directly with Cargo
cargo install shurly

# Run locally
cargo run

# Docker build (running is the same)
docker build --tag shurly .
```

A simple `docker compose up` will get you started. Use `docker compose up
--build` to rebuild the Shurly image.

> The Docker Compose setup also runs the Docker version of Shurly, this might
> not be ideal for fast development iterations. Docker Compose provides the
> option to only run a single container of the setup.
>
> ```sh
> # `the-data` is the name of the PostgreSQL service
> docker compose up the-data
> ```

When running it without Docker, there needs to be a `DATABASE_URL` environment
variable.

## Pre-built Docker images

For those who like to use Docker, but don't want to go through the hassle of
building the images; pre-built images are available:

-   [`ghcr.io/workplacebuddy/shurly:master`]
-   More information and tags available here:
    <https://github.com/workplacebuddy/shurly/pkgs/container/shurly>

## Configuration

When running with the defaults, missing configuration has a sane default oris
automatically generated. For development this is fine, but running it in
production has a different set of requirements. All configuration is done
through environment variables.

### Setup logging

[`tracing`] is used for all logging (optional)

```sh
RUST_LOG=shurly=debug,tower_http=debug
```

### Encoding secrets

Secret for encoding JWT tokens, make sure this is long enough (optional,
default: some random string)

```sh
JWT_SECRET=
```

### Database connection

Connection string for `PostgreSQL` server.
When running the Docker Compose setup this will be provided.

```sh
DATABASE_URL=
```

### The actual server

To communicate with the outside world, Shurly needs to bind to an address to
accept connections.

```sh
# Address for Shurly to bind to
ADDRESS=0.0.0.0:7000

# Override just the port to run Shurly on
PORT=7000
```

### Initial user credentials

On the first run there is a user created with some randomly generated
credentials; These credentials are displayed in the server log. The initial
credentials can be changed with the `INITIAL_USERNAME` and `INITIAL_PASSWORD`
environment variables. When using these variables, they will not be output to
the log.

-   `INITIAL_USERNAME`: Username of the first user for the first run (optional,
    default: some `UUIDv4`)
-   `INITIAL_PASSWORD`: Password of the first user for the first run (optional,
    default: something random)

The environment variables can be set in a `.env` file, see `.env.default` for
an example.

## `PostgreSQL` database migrations

Shurly uses `SQLx` for all `PostgreSQL` database interactions, migrations are
run automatically on start up.

The migration files can be found in `./migrations` and are sorted on filename.

### Development

Working with migrations can be a bit of a hassle, the project does not build
properly without a valid database connection and the right schema. The Docker
image uses the `SQLX_OFFLINE=true` environment variable to use the cached data
inside the `sqlx-data.json` file. Running a compiled version of Shurly will
automatically run the migrations -- getting it compiled is the trick :)

Using the `SQLx` CLI adds a couple of nicities to work with migrations.

-   Install with: `cargo install sqlx-cli`, or with [other options].
-   Make sure the `DATABASE_URL` environment variable is set
-   To start: `cargo sqlx migrate run`
-   To revert: `cargo sqlx migrate revert`

#### Rust version

Shurly is tested on Rust version 1.85, stable and beta. The Docker images are
created with the `rust:1.85-slim` base image.

# Things to to (maybe)

-   [ ] Endpoints to expose some statistics, data is already captured
-   [ ] Track incoming parameters in `hits`, maybe?
-   [ ] Add aliases for destinations, so hits count for the original
-   [x] A somewhat attractive 404 page, ~~or a default destination?~~
-   [ ] Description of all the API endpoints
-   [x] Run tests on an actual database

> And, don't call me Shirly.

[docker]: https://www.docker.com/
[docker compose]: https://docs.docker.com/compose/
[`tracing`]: https://lib.rs/crates/tracing
[other options]: https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md
[`ghcr.io/workplacebuddy/shurly:master`]: https://github.com/workplacebuddy/shurly/pkgs/container/shurly
