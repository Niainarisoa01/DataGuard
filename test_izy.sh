#!/bin/bash

export DATABASE_URL=postgres://postgres:672643@localhost:5432/dataguard

echo "Building API..."
source $HOME/.cargo/env
cargo build --release -p api > /dev/null 2>&1

echo "Starting API..."
target/release/api > /tmp/api_log.txt 2>&1 &
API_PID=$!

trap "kill $API_PID" EXIT

# Wait for API
for i in {1..10}; do
  if curl -s http://localhost:3000/health > /dev/null; then
    break
  fi
  sleep 1
done

# Get Account ID
ACCOUNT_ID=$(psql $DATABASE_URL -q -A -t -c "SELECT id FROM accounts LIMIT 1;")

# Create Schema for IZY MISAM
SCHEMA_NAME="IZY_MISAM_JS_$(date +%s)"
cat <<EOF > /tmp/izy_schema.json
{
  "account_id": "$ACCOUNT_ID",
  "name": "$SCHEMA_NAME",
  "description": "Validation for student records from Excel",
  "json_schema": {
    "type": "object",
    "properties": {
      "Nom": { "type": "string" },
      "Sexe": { "type": "string", "enum": ["MASCULIN", "FEMININ"] },
      "Niv_etu": { "type": "string" }
    },
    "required": ["Nom", "Sexe", "Niv_etu"]
  }
}
EOF

echo "Registering schema..."
SCHEMA_RESPONSE=$(curl -s -X POST http://localhost:3000/v1/schemas \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: test_key" \
  -d @/tmp/izy_schema.json)

SCHEMA_ID=$(echo "$SCHEMA_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin).get('id', ''))")

if [ -z "$SCHEMA_ID" ]; then
    echo "Failed to create schema. Response: $SCHEMA_RESPONSE"
    exit 1
fi

echo "Running validation on /tmp/izy_misam.csv using Schema ID: $SCHEMA_ID"
curl -s -X POST "http://localhost:3000/v1/validate/csv/$SCHEMA_ID" \
  -H "X-Api-Key: test_key" \
  -F "file=@/tmp/izy_misam.csv" \
  -o /tmp/izy_errors.csv

echo "Results summary:"
ERROR_COUNT=$(wc -l < /tmp/izy_errors.csv)
echo "Total error lines: $ERROR_COUNT"
if [ "$ERROR_COUNT" -gt 1 ]; then
    echo "First few errors:"
    head -n 5 /tmp/izy_errors.csv
else
    echo "No errors found (or only header)!"
fi

echo "Checking database usage logs..."
psql $DATABASE_URL -c "SELECT endpoint, records_processed, duration_ms FROM usage_logs ORDER BY created_at DESC LIMIT 1;"
