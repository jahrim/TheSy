# !/bin/bash

# Usage: ./test-bench.sh <benchmark> <output-file> [timeout-seconds]
#
# Description:
# Runs the given benchmark repeatedly until it either succeeds or fails
# with an error (non-zero exit code). Each attempt is given the specified
# timeout (default: 60 seconds). Output (stdout and stderr) is written to
# the specified output file.
#
# As TheSy is non-deterministic, this script helps in testing by retrying
# benchmarks that may time out or fail intermittently.

benchmark=$1
output_file=$2
timeout=${3-60}

trap 'echo; echo "Interrupted."; exit 130' INT

iteration=0
while true; do
    iteration=$((iteration + 1))
    printf "Attempt ${iteration}..."
    timeout --foreground $timeout \
        cargo run \
            --release \
            -- --egraph-type versioned \
               --max-memory 32 \
               --no-output-files \
               ${benchmark} > ${output_file} 2>&1
    
    status=$?
    if [ "$status" -eq 124 ]; then
        printf "Timed out.\n"
        continue
    fi
    if [ "$status" -eq 0 ]; then
        printf "Success.\n"
        continue
    elif [ "$status" -ne 0 ]; then
        printf "Failed with exit code $status\n"
        break
    fi
done