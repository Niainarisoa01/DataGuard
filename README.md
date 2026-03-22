# DataGuard API 🛡️

A high-performance JSON and CSV data validation/transformation API built in Rust. It serves as a micro-service designed to clean, validate, and transform incoming data pipelines (webhooks, CSV uploads, etc.) before they hit your core systems.

## Features

- **Blazing Fast**: Validates payloads in < 30ms with < 15MB RAM footprint.
- **JSON Schema Validation**: Full `draft-07` and `2020-12` support.
- **CSV Batching**: Streaming validation of large files.
- **Transformations**: Declarative data mapping and conversions.
- **Webhooks**: Asynchronous alerts on invalid schemas.

## Components

The workspace is divided into three crates:
- `api/`: Main Axum HTTP server and endpoints.
- `worker/`: Background jobs processor for async webhooks and batch tasks.
- `shared/`: Database models, schema engines, and core business logic.

## Setup

Developed using stable Rust.

```bash
cargo build
cargo test
```

## License
MIT
