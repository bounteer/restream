# Multi-stage build for Rust application
FROM rust:1.91 as builder

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy main to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this will be cached)
RUN cargo build --release

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage with minimal base image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false appuser

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/restream /app/restream

# Change ownership to app user
RUN chown appuser:appuser /app/restream

# Switch to app user
USER appuser

# Expose port
EXPOSE 8000

# Run the application
CMD ["./restream"]
