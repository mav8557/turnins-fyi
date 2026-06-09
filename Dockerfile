FROM rust:1.82-slim AS builder
WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/
COPY shared/Cargo.toml shared/
COPY frontend/Cargo.toml frontend/
# Stub crates so dependency download compiles
RUN mkdir -p backend/src shared/src frontend/src \
 && echo 'fn main(){}' > backend/src/main.rs \
 && echo '' > shared/src/lib.rs \
 && echo 'fn main(){}' > frontend/src/main.rs \
 && cargo build --release --package backend \
 && rm -rf backend/src shared/src frontend/src

# Build real source
COPY backend/src backend/src
COPY shared/src shared/src
COPY data/items.json data/items.json
RUN touch backend/src/main.rs \
 && cargo build --release --package backend

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/turnins-backend
EXPOSE 8080
CMD ["turnins-backend"]
