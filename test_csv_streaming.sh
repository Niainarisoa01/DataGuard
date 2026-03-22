#!/bin/bash

export DATABASE_URL=postgres://postgres:672643@localhost:5432/dataguard

echo "Building and starting API..."
source $HOME/.cargo/env
cargo build --release -p api
target/release/api &
API_PID=$!

echo "Waiting for API..."
while ! curl -s http://localhost:3000/health > /dev/null; do
  sleep 1
done

echo "Fetching a valid schema ID from DB..."
SCHEMA_ID=$(psql $DATABASE_URL -q -A -t -c "SELECT id FROM schemas LIMIT 1;")

if [ -z "$SCHEMA_ID" ]; then
    echo "No schema found. Run benchmark.sh first!"
    kill $API_PID
    exit 1
fi

echo "============================================="
echo "Testing Streaming API with 200,000 rows CSV"
echo "URL: http://localhost:3000/v1/validate/csv/$SCHEMA_ID"
echo "============================================="

time curl -X POST http://localhost:3000/v1/validate/csv/$SCHEMA_ID \
  -H "X-Api-Key: test_key" \
  -F "file=@/tmp/massive_test.csv" \
  -o /tmp/errors_output.csv

echo "============================================="
echo "Done! Counting errors..."
ERROR_LINES=$(wc -l < /tmp/errors_output.csv)
echo "Total lines in error report: $ERROR_LINES"

echo "Checking the first 5 errors returned:"
head -n 5 /tmp/errors_output.csv

echo "============================================="
echo "Checking UsageLogs from DB:"
psql $DATABASE_URL -c "SELECT endpoint, status_code, records_processed, duration_ms FROM usage_logs ORDER BY created_at DESC LIMIT 1;"

kill $API_PID
