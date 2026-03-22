#!/bin/bash

# Configuration
export DATABASE_URL=postgres://postgres:672643@localhost:5432/dataguard
TOTAL_REQUESTS=2000000
CONCURRENCY=500

echo "1. Installing 'oha' (HTTP load tester) if missing..."
if ! command -v oha &> /dev/null; then
    source $HOME/.cargo/env && cargo install oha
fi

echo ""
echo "2. Compiling and Starting API in RELEASE mode (for max performance)..."
source $HOME/.cargo/env
cargo build --release -p api
target/release/api &
API_PID=$!

echo "Waiting for API to be ready..."
while ! curl -s http://localhost:3000/health > /dev/null; do
  sleep 1
done

echo ""
echo "3. Preparing test schema and payload..."
ACCOUNT_ID=$(psql $DATABASE_URL -q -A -t -c "SELECT id FROM accounts WHERE email='test@dataguard.com' LIMIT 1;")

cat <<EOF > /tmp/schema_payload.json
{
  "account_id": "$ACCOUNT_ID",
  "name": "perf_test_schema_$RANDOM",
  "description": "Load Testing Schema",
  "json_schema": {
    "type": "object",
    "properties": {
      "username": { "type": "string", "minLength": 3 },
      "age": { "type": "integer", "minimum": 18 }
    },
    "required": ["username", "age"]
  }
}
EOF

SCHEMA_RESPONSE=$(curl -s -X POST http://localhost:3000/v1/schemas \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: test_key_123" \
  -d @/tmp/schema_payload.json)

SCHEMA_ID=$(echo "$SCHEMA_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin)['id'])")

cat <<EOF > /tmp/valid_payload.json
{
  "schema_id": "$SCHEMA_ID",
  "data": {
    "username": "performance_tester",
    "age": 28
  }
}
EOF

echo ""
echo "=========================================================="
echo "🚀 4. FIRE! RUNNING LOAD TEST: $TOTAL_REQUESTS requests"
echo "=========================================================="
oha -n $TOTAL_REQUESTS -c $CONCURRENCY -m POST -T application/json -a "X-Api-Key: load_test_key" -d "$(cat /tmp/valid_payload.json)" http://localhost:3000/v1/validate

echo ""
echo "Cleaning up..."
kill $API_PID
