# Builds havenkeys-server, the only deployable in this repository. It lives at
# the root because that is where a platform looks for a Dockerfile, and because
# the build needs the whole Cargo workspace as context anyway:
#
#   docker build -t havenkeys-server .
#
# The desktop app and the extension are not built here; they ship as installers
# and as an unpacked extension (see docs/development.md).
#
# Nothing secret enters the image. There is no DATABASE_URL at build time —
# the crate builds every query at runtime precisely so that stays true — and
# no key material is ever baked in.

FROM rust:1.88-slim-bookworm AS builder
WORKDIR /src

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

# The workspace's other members are not built, but cargo has to read their
# manifests to resolve the workspace, so the whole tree is copied.
COPY . .
RUN cargo build --release -p havenkeys-server \
 && strip target/release/havenkeys-server

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --no-create-home --uid 10001 havenkeys

COPY --from=builder /src/target/release/havenkeys-server /usr/local/bin/havenkeys-server

USER havenkeys
EXPOSE 8080
ENV RUST_LOG=havenkeys_server=info,tower_http=info
ENTRYPOINT ["/usr/local/bin/havenkeys-server"]
