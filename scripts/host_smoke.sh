#!/bin/bash
# Smoke test for the host runtime/compositor stack. Default backend is in-memory.

TIMEOUT_SECONDS=${TIMEOUT_SECONDS:-120}
BACKEND_RAW=${GRAPH_BACKEND:-in-memory}
BACKEND=$(echo "$BACKEND_RAW" | tr '[:upper:]' '[:lower:]')
LOG_FILE="host_smoke.log"

# Clean up previous log
rm -f "$LOG_FILE"

# Run the host stack with a timeout so it does not hang CI.
GRAPH_BACKEND="$BACKEND" timeout "$TIMEOUT_SECONDS" make run-host > "$LOG_FILE" 2>&1
EXIT_CODE=$?

EXPECTED_BACKEND="Graph backend: InMemory"
if [ "$BACKEND" = "neo4j" ]; then
    EXPECTED_BACKEND="Graph backend: Neo4j"
fi

# Stop the database container if we started it.
if [ "$BACKEND" = "neo4j" ]; then
    docker compose -f docker-compose.neo4j.yml down >/dev/null 2>&1
fi

HAS_HTTP=$(grep -F "Host compositor: http://127.0.0.1:8080/" "$LOG_FILE" || true)
HAS_BACKEND=$(grep -F "$EXPECTED_BACKEND" "$LOG_FILE" || true)

if [ -n "$HAS_HTTP" ] && [ -n "$HAS_BACKEND" ]; then
    echo "SUCCESS: host runtime reached HTTP server with backend=$BACKEND."
    exit 0
fi

echo "FAILURE: host runtime did not start cleanly (backend=$BACKEND)."
if [ $EXIT_CODE -eq 124 ]; then
    echo "Timed out after ${TIMEOUT_SECONDS}s waiting for startup."
fi
echo "--- host_smoke.log (last 40 lines) ---"
if [ -f "$LOG_FILE" ]; then
    tail -n 40 "$LOG_FILE"
else
    echo "No log file was produced."
fi
echo "--------------------------------------"
exit 1
