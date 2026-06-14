# SKILLS.md - Project Skills & Technical Environment

## 1. System Context & Stack
* **Project Name**: Shabakat Server
* **Core Tech Stack**: Rust (Tokio, Axum v0.7), SQLite (`rusqlite` v0.32.1), React/TypeScript static bundle, Docker.
* **Target Run Environment**: Asustor Lockerstor Gen 1 Host NAS (Intel Celeron J4125 CPU, x86_64 headless Linux).
* **Package Manager / Build Tools**: cargo, docker-compose.

## 2. Allowed CLI & Maintenance Commands
The User can safely copy and execute the following commands when explicitly instructed:
- **Development / Build Run**: `cargo check` | `cargo build` | `cargo run`
- **Container Control**: `docker compose up --build` | `docker compose down` | `docker logs -f shabakat-server`
- **Testing Loop**: `curl http://localhost:7779/api/health`

## 3. Code Style & Safety Standards
- All database access operations MUST be wrapped inside `tokio::task::spawn_blocking` to protect the Celeron CPU from reactor thread starvation.
- All logs related to system actions, alerts, or processing stages must use the `[FLIGHT_RECORDER]` prefix.
- No `unwrap()` statements are allowed on production code execution paths.

---

## 4. Architectural Boundaries & Constraints
- **Headless Isolation**: This is a standalone server application; never mix Tauri workspace modules or Android JNI bindings into this repository.
- **Absolute Directory Isolation**: All modifications must occur inside `~/Documents/ShabakatServer`. Never touch, analyze, or pull from the legacy client directory `~/Documents/Shabakat`.
