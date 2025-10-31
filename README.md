# Restream: play transcript in websocket

Input: Transcript
Output: websocket message

Use Case: testing environment for any real-time voice application.

## Prerequisites

- Docker and Docker Compose
- Git

## Quick Start with Docker

### Build and Run

```bash
# Build and start the container
docker-compose up --build

# Run in detached mode
docker-compose up -d --build
```

### Stop the Application

```bash
# Stop the running container
docker-compose down
```

### View Logs

```bash
# View real-time logs
docker-compose logs -f restream

# View logs without following
docker-compose logs restream
```

### Development Commands

```bash
# Rebuild after code changes
docker-compose up --build

# Access the running container
docker-compose exec restream /bin/bash

# Remove containers and images
docker-compose down --rmi all
```

## Configuration

The application runs on port 8000 by default. The Docker setup includes:

- Multi-stage build for optimized image size
- Non-root user for security
- Automatic restart unless stopped
- Logging level set to `info`

## Local Development

For local development without Docker:

```bash
# Build the project
cargo build

# Run the application
cargo run

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt
```
