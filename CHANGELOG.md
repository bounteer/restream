# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0]

### Modified
- changed rewind endpoint as get, with default value a 'intake_call.csv'


## [0.1.0]

### Added
- WebSocket-based transcript rewind system with session management
- Session-based broadcasting of transcript messages
- Automatic session cleanup after message broadcasting completes
- WebSocket server on port 3031 for real-time transcript streaming
- UUID-based session identification system

### Changed
- Modified `/rewind` endpoint to accept `filename` parameter instead of `transcript_id`
- Updated rewind response to return WebSocket connection information (`websocket_url`, `session_id`, `port`)
- Enhanced rewind functionality to create isolated sessions for each transcript playback

### Added Dependencies
- `tokio-tungstenite` 0.20 - WebSocket server implementation
- `uuid` 1.0 with v4 features - Session ID generation
- `futures-util` 0.3 - Async stream utilities

### Technical Details
- Each rewind request creates a unique session stored in memory
- WebSocket clients connect using session ID for message broadcasting
- Messages are broadcasted sequentially with 100ms intervals
- Sessions are automatically destroyed when all messages are sent
- Dual server architecture: HTTP API on port 3030, WebSocket on port 3031