#!/bin/bash
TIMEOUT_SECONDS=20
EXPECTED_TEXT="Kernel started!"
SERIAL_LOG="smoke_serial.log"
LOG_FILE="smoke_test.log"

# Clean up previous logs
rm -f "$SERIAL_LOG" "$LOG_FILE"

# Run make run with a timeout.
# We configure QEMU to write serial output to a file directly.
# We still capture stdout/stderr of make to smoke_test.log for build errors.
timeout "$TIMEOUT_SECONDS" make run QEMUFLAGS="-m 2G -serial file:$SERIAL_LOG -display none" > "$LOG_FILE" 2>&1

# Append serial log to main log for visibility
echo "=== Serial Output ===" >> "$LOG_FILE"
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG" >> "$LOG_FILE"
else
    echo "No serial log generated." >> "$LOG_FILE"
fi

# Check if the expected text is in the serial log
if grep -Fq "$EXPECTED_TEXT" "$SERIAL_LOG"; then
    echo "✅ SUCCESS: Found '$EXPECTED_TEXT' in boot logs."
    exit 0
else
    echo "❌ FAILURE: '$EXPECTED_TEXT' not found in boot logs."
    echo "--- Serial Log Output (Last 20 lines) ---"
    if [ -f "$SERIAL_LOG" ]; then
        tail -n 20 "$SERIAL_LOG"
    else
        echo "No serial log found."
    fi
    echo "-----------------------------------------"
    exit 1
fi
