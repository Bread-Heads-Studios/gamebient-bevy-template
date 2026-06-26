# ── Build stage ───────────────────────────────────────────────────────────
# Compiles the WASM bundle and assembles dist/ from source, so the image no
# longer depends on a pre-built dist/ being committed to the repo. The wasm-
# bindgen CLI version is pinned to match the wasm-bindgen library (see
# install.sh / README) — a version skew here produces a bundle that fails to
# load at runtime.
FROM rust:1-bookworm AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends binaryen nodejs \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli@0.2.108 --locked

WORKDIR /src
COPY . .
RUN bash build_web.sh

# ── Runtime stage ─────────────────────────────────────────────────────────
FROM nginx:alpine

RUN rm /etc/nginx/conf.d/default.conf

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/dist/ /usr/share/nginx/html/

EXPOSE 8080
