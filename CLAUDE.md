# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust project named "casset" using the 2024 edition. The project is currently minimal with only a basic main.rs file containing a "Hello, world!" program.

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

## Project Structure

The project follows standard Rust conventions:
- `Cargo.toml` - Project configuration and dependencies
- `src/main.rs` - Main entry point (currently contains basic Hello World)
- `target/` - Build artifacts (ignored by git)

## Architecture Notes

This is a new/minimal Rust project with standard binary crate structure. The codebase currently consists of just a main function that prints "Hello, world!".