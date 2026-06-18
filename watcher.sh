#!/bin/bash
# E2E test watcher — tails the log and exits when output stops for >120s
LOG="$1"
POLL="${2:-30}"
IDLE_LIMIT="${3:-120}"

if [ -z "$LOG" ]; then
    echo "Usage: $0 <logfile> [poll_secs=30] [idle_limit_secs=120]"
    exit 1
fi

echo "Watching $LOG (poll=${POLL}s, idle_limit=${IDLE_LIMIT}s)"
echo "Key phases to watch for:"
echo "  [migrate] MIGRATION COMPLETED"
echo "  [e2e] Host-side disk validation"
echo "  [e2e] Re-launching VM"
echo "  [vm-post] Welcome to (composefs post-reboot)"
echo "  [e2e] Booted backend: ComposeFS"
echo "  [e2e] === Running OSTree rollback test"
echo "  [e2e] === Running commit cleanup test"
echo "---"

LAST_SIZE=0
IDLE_SECONDS=0

while true; do
    if [ -f "$LOG" ]; then
        CURRENT_SIZE=$(stat -c%s "$LOG" 2>/dev/null || echo 0)
        
        if [ "$CURRENT_SIZE" != "$LAST_SIZE" ]; then
            # Log grew — print new lines
            tail -n +$((LAST_SIZE == 0 ? 0 : 1)) "$LOG" 2>/dev/null | tail -c +$((LAST_SIZE + 1)) 2>/dev/null
            LAST_SIZE=$CURRENT_SIZE
            IDLE_SECONDS=0
        else
            IDLE_SECONDS=$((IDLE_SECONDS + POLL))
        fi
    fi
    
    # Check if the script process is still running
    if ! pgrep -f 'run-e2e.sh' > /dev/null 2>&1; then
        echo ""
        echo "=== E2E SCRIPT EXITED ==="
        tail -5 "$LOG"
        exit 0
    fi
    
    if [ "$IDLE_SECONDS" -ge "$IDLE_LIMIT" ]; then
        echo ""
        echo "=== IDLE TIMEOUT (${IDLE_LIMIT}s without output) ==="
        echo "Last lines:"
        tail -5 "$LOG"
        echo "Processes:"
        ps aux | grep -E 'run-e2e|qemu-system' | grep -v grep
        exit 1
    fi
    
    sleep "$POLL"
done
