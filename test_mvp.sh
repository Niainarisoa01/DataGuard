#!/bin/bash

export DATABASE_URL=postgres://postgres:672643@localhost:5432/dataguard

echo "1. Seeding test account..."
psql $DATABASE_URL -q -c "INSERT INTO accounts (email, plan) VALUES ('test@dataguard.com', 'pro') ON CONFLICT (email) DO NOTHING;" > /dev/null
ACCOUNT_ID=$(psql $DATABASE_URL -q -A -t -c "SELECT id FROM accounts WHERE email='test@dataguard.com' LIMIT 1;")
echo "Account ID created/found: $ACCOUNT_ID"

echo ""
echo "2. Starting the API server in the background (compiling may take a moment)..."
cargo run -p api &
API_PID=$!

echo "Waiting for API to be ready..."
while ! curl -s http://localhost:3000/health > /dev/null; do
  sleep 1
done
echo "API is ready!"

echo ""
echo "3. Creating a new JSON Schema via API (POST /v1/schemas)..."
cat <<EOF > /tmp/schema_payload.json
{
  "account_id": "$ACCOUNT_ID",
  "name": "user_profile",
  "description": "Validation schema for user profiles",
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

echo "Response from creating schema:"
echo "$SCHEMA_RESPONSE"

# Extract ID using python instead of jq (which is built-in standard)
SCHEMA_ID=$(echo "$SCHEMA_RESPONSE" | python3 -c "import sys, json; print(json.load(sys.stdin)['id'])")
echo "Extracted SCHEMA_ID: $SCHEMA_ID"

echo ""
echo "4. Validating a VALID payload (POST /v1/validate)..."
curl -s -X POST http://localhost:3000/v1/validate \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: test_key_123" \
  -d "{
    \"schema_id\": \"$SCHEMA_ID\",
    \"data\": {
      \"username\": \"alice123\",
      \"age\": 25
    }
  }"

echo ""
echo ""
echo "5. Validating an INVALID payload (POST /v1/validate)..."
curl -s -X POST http://localhost:3000/v1/validate \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: test_key_123" \
  -d "{
    \"schema_id\": \"$SCHEMA_ID\",
    \"data\": {
      \"username\": \"bo\",
      \"age\": 15
    }
  }"

echo ""
echo "Cleaning up server..."
kill $API_PID
