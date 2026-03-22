# DataGuard API 🛡️

![Build Status](https://img.shields.io/github/actions/workflow/status/Niainarisoa01/DataGuard/ci.yml?branch=main)
![Rust](https://img.shields.io/badge/Rust-1.74%2B-orange)
![License](https://img.shields.io/badge/License-MIT-blue)

A high-performance JSON and CSV data validation/transformation API built in Rust. DataGuard serves as a micro-service designed to clean, validate, and transform incoming data pipelines (webhooks, CSV uploads, streams) before they hit your core systems.

## 🚀 Key Features

- **Blazing Fast**: Validates payloads in < 50ms at p95 with a minimal RAM footprint, capable of handling up to 10,000 req/s.
- **JSON Schema Validation**: Full validation using the `jsonschema` engine with support for draft-07, 2019-09, and 2020-12.
- **CSV Batching**: Multipart streaming validation of massive CSV files (up to 1GB) without crashing memory limits.
- **Transformations**: Declarative data mapping and conversions directly handled via `x-transform` extensions.
- **Webhooks**: Asynchronous alerts on invalid schemas directly pushed to your integrated systems.
- **Universal Agnostic REST API**: Easily interact via cURL, Node.js, Python, Go, PHP, or any HTTP client.

## 🏗️ Architecture & Crates

The workspace is heavily optimized and divided into three independent crates:
- `api/`: The main Axum HTTP server exposing endpoints for validation, batch processing, and schema management.
- `worker/`: Background jobs processor powered by Tokio, handling asynchronous webhooks and retry logic.
- `shared/`: Shared database abstractions (`sqlx`), models, caching layers, and core business logic that both the `api` and `worker` consume.

## 🛠️ Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [PostgreSQL](https://www.postgresql.org/) (Running instances for local dev / testing)
- [Docker](https://www.docker.com/) (Optional, for running with `docker-compose`)

### Installation & Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/Niainarisoa01/DataGuard.git
   cd DataGuard
   ```

2. **Environment Variables:**
   Copy the `.env.example` to `.env` (or create one):
   ```bash
   DATABASE_URL=postgres://postgres:password@localhost:5432/dataguard
   PORT=3000
   ```

3. **Run database migrations (SQLx):**
   ```bash
   cargo install sqlx-cli
   cargo sqlx database setup
   # Note: The project uses SQLX_OFFLINE=true in CI using the .sqlx directory cache
   ```

4. **Run the server:**
   ```bash
   cargo run -p api
   ```

## 📖 API Usage Defaults

All endpoints require an API Key header: `X-Api-Key: dg_live_...`

### Synchronous JSON Validation
```bash
curl -X POST http://localhost:3000/v1/validate \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: your_api_key_here" \
  -d '{
    "schema_id": "user-signup-v3",
    "data": { "email": "bob@example.com", "age": 25 }
  }'
```

### Batch CSV Validation
```bash
curl -X POST http://localhost:3000/v1/validate-batch \
  -H "X-Api-Key: your_api_key_here" \
  -F "schema_id=product-catalog" \
  -F "file=@products.csv"
```

## 🧪 Testing and CI

The repository contains an automated GitHub Actions pipeline. If you make modifications to the database schema (`sqlx` queries), make sure to prepare the offline SQLx data before committing:
```bash
cargo sqlx prepare --workspace
```
To run tests locally:
```bash
cargo test --all-features
```

## 📄 License
This project is licensed under the MIT License.
