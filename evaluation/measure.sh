#!/bin/bash

### 1. Description
### Repeatedly runs a binary and collects average performance metrics.
### 2. Usage
### - ./measure.sh "gtime -v ./command.sh" 3 5
###   run `command.sh` 3 times (0 means indefinitely) for at most 5 seconds in total (0 means 
###   indefinitely), and return the average time and memory of a run.

if [[ -z "$1" ]]; then
    echo "Usage: $0 <measure & binary>"
    exit 1
fi

binary=$1
repetitions=${2:-0}  # Number of repetitions (default: 0, meaning indefinitely)
maxtime=${3:-0}      # Maximum time in seconds (default: 0, meaning indefinitely)

code=0               # Maximum exit code of the runs (0 means success)
seconds=0            # Total time in seconds
kilobytes=0          # Total memory in kilobytes
branches=0           # Total branches (i.e. versions) explored
iterations=0         # Number of runs executed

sum() {
    echo "$1 + $2" | bc
}
div() { 
    decimals=$3
    echo "scale=$decimals; $1 / $2" | bc; 
}
max() { 
    if [ $1 -gt $2 ]; then echo $1; else echo $2; fi 
}
echo_output() {
    if [[ $iterations -ne 0 ]]; then
        seconds=$(div $seconds $iterations 2)
        kilobytes=$(div $kilobytes $iterations 2)
        branches=$(div $branches $iterations 2)
    fi
    echo "$code,$seconds,$kilobytes,$branches"
    exit 0
}

# Hook on termination signals (including timeout and Ctrl+C)
trap echo_output SIGTERM SIGINT SIGHUP 

# Run the binary repeatedly
start=$(date +%s)
while [[ $repetitions -eq 0 || $repetitions -gt 0 ]]; do
    out_i=$(bash -c "$binary 2>&1")
    code_i=$?
    seconds_i=$(echo "$out_i" | grep "User time" | head -n 1 | cut -d: -f2 | xargs)
    kilobytes_i=$(echo "$out_i" | grep "Maximum resident set size" | head -n 1 | cut -d: -f2 | xargs)
    branches_i=$(echo "$out_i" | grep "Branches" | head -n 1 | cut -d: -f2 | xargs)
    if [[ -z "$seconds_i" ]]; then
        seconds_i=0
    fi
    if [[ -z "$kilobytes_i" ]]; then
        kilobytes_i=0
    fi
    if [[ -z "$branches_i" ]]; then
        branches_i=0
    fi
    code=$(max $code $code_i)
    seconds=$(sum $seconds $seconds_i)
    kilobytes=$(sum $kilobytes $kilobytes_i)
    branches=$(sum $branches $branches_i)
    ((iterations++))

    # Termination Conditions
    if [[ $code -ne 0 ]]; then
        break
    fi

    if [[ $repetitions -ne 0 ]]; then
        ((repetitions--))
        if [[ $repetitions -eq 0 ]]; then
            break
        fi
    fi

    end=$(date +%s)
    elapsed=$((end - start))
    if [[ $maxtime -ne 0 && $elapsed -ge $maxtime ]]; then
        break
    fi
done

# Output metrics
echo_output