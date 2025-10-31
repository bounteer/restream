# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust project named "restream" using the 2024 edition. It's a websocket application that plays transcripts for testing real-time voice applications. The project includes Docker containerization with multi-stage builds and runs on port 8080 (HTTP) and port 8081 (WebSocket).

## Development Commands

### Build and Run
- `cargo build` - Compile the project
- `cargo run` - Build and run the executable
- `cargo check` - Check code for errors without building

### Testing
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run a specific test

### Code Quality
- `cargo clippy` - Run the Clippy linter (if available)
- `cargo fmt` - Format code (if available)

### Docker Development
- `docker-compose up --build` - Build and start the container
- `docker-compose up -d --build` - Build and start in detached mode
- `docker-compose down` - Stop the running container
- `docker-compose logs -f restream` - View real-time logs
- `docker-compose exec restream /bin/bash` - Access running container

## Project Structure

The project follows standard Rust conventions:
- `Cargo.toml` - Project configuration and dependencies
- `src/` - Source code directory
- `Dockerfile` - Multi-stage Docker build configuration
- `docker-compose.yaml` - Container orchestration setup
- `target/` - Build artifacts (ignored by git)

## Architecture Notes

This is a Rust websocket application designed for testing real-time voice applications. The project uses:

- **Docker Multi-stage Build**: Optimized container with minimal runtime image
- **Port 8080**: HTTP server port exposed via Docker
- **Port 8081**: WebSocket server port exposed via Docker
- **Security**: Non-root user execution in container
- **Logging**: Configured with RUST_LOG=info environment variable
- **Auto-restart**: Container restarts unless explicitly stopped

The application streams transcript data via websockets to simulate real-time voice input for testing purposes.