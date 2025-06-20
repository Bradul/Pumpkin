#!/bin/bash

MODEL="jobshop.mzn"
SOLVER="pumpkin"
INPUT_DIR="inputs"
OUTPUT_DIR="outputs"
PREFIX="instance"
NUM_TESTS=10

JOBS=(10 15) #15 20 20 30 30 50 50 100)
MACHINES=(10 8) #15 15 20 15 20 15 20 20)

# Create output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

echo "Starting batch tests..."

for i in "${!JOBS[@]}"; do
  j=${JOBS[$i]}
  m=${MACHINES[$i]}
  
  for t in $(seq 1 $NUM_TESTS); do
    FILENAME="${PREFIX}_j${j}m${m}_${t}.dzn"
    INPUT_PATH="${INPUT_DIR}/${FILENAME}"
    OUTPUT_PATH="${OUTPUT_DIR}/output_j${j}m${m}_${t}_ef.txt"
    
    if [[ -f "$INPUT_PATH" ]]; then
      echo "Running test: $FILENAME"
      minizinc -s -a -t 1200000 -o "$OUTPUT_PATH" --output-time --solver "$SOLVER" "$MODEL" "$INPUT_PATH"
    else
      echo "File not found: $INPUT_PATH"
    fi
  done
done

echo "All tests completed."
