# ---- Build stage ----
FROM rust:1.91 AS builder
WORKDIR /app

# Copy manifests first to leverage caching
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build your actual binary (main)
RUN cargo build --release --bin main

# Runtime stage, match builder distro (glibc 2.39) 
FROM debian:trixie-slim 
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/main /app/main
COPY transcript ./transcript

# Use non-root user for security
RUN useradd -r -s /bin/false appuser
USER appuser

EXPOSE 8080
CMD ["./main"]
