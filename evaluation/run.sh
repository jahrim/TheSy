#!/bin/bash

####################################################################################
### 1. Description                                                               ###
### Run the evaluation, reproducing the results of the paper.                    ###       
### 2. Usage                                                                     ###    
### - ./run.sh ./benchmarks.txt gtime 311 16 300 32 0                            ###   
###   run TheSy on the first 311 .th files inside ./benchmarks.txt, measuring    ###
###   time with gtime, with 16 jobs, 300 seconds timeout, and 32 GB max memory.  ###
###   The last 0 means that each benchmark is run indefinitely until timeout; if ###
###   set to 1, each benchmark is run only once.                                 ###
###   Can be interrupted and resumed thanks to GNU Parallel.                     ### 
###   Expected time: ~4.3h with 16 jobs.                                         ###
####################################################################################

# Constants
BINARY=../target/release/TheSy
VARIANTS=(cloning versioned persistent colored)

# Inputs
TIMER=${1:-"/usr/bin/time"}                         # Use `gtime` (MacOS) or `/usr/bin/time` (Linux; Default)
SOURCE=${2:-"benchmarks-sorted-by-runtime.txt"}     # Source file where rows are .th files (default: ./benchmarks-sorted-by-runtime.txt)
BENCH_SIZE=${3:-311}                                # Number of benchmarks to run
JOBS=${4:-16}                                       # Number of parallel jobs (default: 16)    
MAX_TIME=${5:-300}                                  # Max time for each benchmark (default: 300s)  
MAX_MEMORY=${6:-32}                                 # Max memory for each benchmark (default: 32GB)
ONCE=${7:-0}                                        # If set to 1, run each benchmark only once (instead of indefinitely until MAX_TIME)

BENCHMARK_FILES=$(cat $SOURCE)
if [ $BENCH_SIZE -gt 0 ]; then
  BENCHMARK_FILES=$(echo "$BENCHMARK_FILES" | head -n $BENCH_SIZE)
fi

if ! $TIMER --version &>/dev/null; then
  echo "Error: TIMER command '$TIMER' not found or does not support --version." >&2
  exit 1
fi
if [ ! -f "$BINARY" ]; then
  echo "Error: TheSy binary not found at '$BINARY'. Please build TheSy before running this script." >&2
  exit 1
fi

export BINARY
export VARIANTS_STRING="${VARIANTS[*]}"
export TIMER
export MAX_MEMORY
export MAX_TIME
export ONCE

echo "Running TheSy with configuration:"
echo "  binary: ${BINARY}"
echo "  variants: ${VARIANTS[*]}"
echo "  timer: ${TIMER}"
echo "  benchmarks: ${SOURCE} (first ${BENCH_SIZE})"
echo "  jobs: ${JOBS}"
echo "  max time: ${MAX_TIME}s"
echo "  max memory: ${MAX_MEMORY}GB"

# Prepare output directories
mkdir -p .bench
mkdir -p results
mkdir -p results/csv

csv_header="benchmark"
for v in "${VARIANTS[@]}"; do
  csv_header+=",${v}_code,${v}_secs,${v}_kb,${v}_branches"
done
echo "$csv_header" > results/csv/results.csv

# Run evaluation in parallel
handle() { 
  file=$1
  num=$2
  read -ra VARIANTS <<< "$VARIANTS_STRING"

  csv_row="$file"
  for variant in "${VARIANTS[@]}"; do
    cmd="$TIMER -v timeout --foreground -k 1 $MAX_TIME $BINARY --egraph-type $variant --max-memory $MAX_MEMORY --no-output-files $file"
    stats=$(./measure.sh "$cmd" $ONCE $MAX_TIME)
    csv_row+=",${stats}"
  done
  echo "$csv_row" > .bench/results.$num.csv
}
export -f handle
parallel --bar -j $JOBS --joblog .bench/parallel.log --resume handle {} "{#}" ::: $BENCHMARK_FILES
cat .bench/results.*.csv >> results/csv/results.csv

# Cleanup
echo "Cleaning up temporary files..."
rm -rf .bench

echo "TheSy evaluation completed. Results saved to results/csv/results.csv"