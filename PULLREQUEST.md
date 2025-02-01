# Release v0.0.1 - Tauri Migration & First Stable Build

## Overview
This release marks our transition from Electron to Tauri and delivers the first packaged executables. The changes focus on performance improvements and native integration capabilities.

## Changelog

### Frontend Architecture
- **Migrated from Electron to Tauri** (#123)
  - Native audio capture via Rust
  - 60% smaller installation size
  - System tray integration
  - Automatic updates
- Removed 600MB+ of Electron dependencies
- Added universal binary support (Apple Silicon/Intel)

### Documentation Updates
- Updated `README.md` with:
  - New installation instructions
  - Tauri architecture diagram
  - Release notes section
  - Cross-platform build targets
- Created `docs/TAURI_MIGRATION.md` guide
- Revised `docs/ARCHITECTURE.md` to reflect new flow

### Backend Changes
- Added Rust-based audio service
- Modified FastAPI endpoints for Tauri IPC
- Implemented chunked upload protocol

### Security
- Implemented process isolation
- Added memory-safe audio processing
- Removed Node.js dependency surface

## Verification Steps

```bash
# Build verification
cargo check --release
cargo tauri build --target universal-apple-darwin

# Installation test
hdiutil verify meeting-minutes_0.0.1_universal.dmg

# Runtime checks
./src-tauri/target/release/meeting-minutes --validate
```

## Known Issues
- ~~Linux AppImage notarization pending~~ (fixed in #124)
- Windows audio capture requires manual driver setup

## Migration Notes
Electron users must:
1. Uninstall previous versions
2. Remove all `node_modules/`
3. Install Rust toolchain
4. Follow new [setup guide](docs/INSTALL.md)

## Screenshots
![New System Tray](docs/screenshots/tray-v0.0.1.png)
![Install Size Comparison](docs/screenshots/install-size.png)

## Contributors
- @sujith - Tauri migration
- @backend-team - FastAPI modifications
- @qa-team - Cross-platform testing

---

**Full Changelog**: https://github.com/Zackriya-Solutions/meeting-minutes/compare/v0.0.0...v0.0.1
