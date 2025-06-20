#!/bin/bash

# Run Pumpklin with explanations on jobshop instances
MODEL="jobshop.mzn"
SOLVER="pumpkin"
INPUT_DIR="inputs"
OUTPUT_DIR="outputs"
NUM_TESTS=40

# Create output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

echo "Starting batch tests..."
echo "Starting LA tests..."

for i in $(seq 1 $NUM_TESTS); do
  FILENAME=$(printf "la%02d" "$i")
  FILENAMEDZN="${FILENAME}.dzn"
  INPUT_PATH="${INPUT_DIR}/${FILENAMEDZN}"
  OUTPUT_PATH="${OUTPUT_DIR}/output_${FILENAME}_ef.txt"

  if [[ -f "$INPUT_PATH" ]]; then
    echo "Running test: $FILENAME"
    minizinc -s -a -t 1200000 -o "$OUTPUT_PATH" --output-time --solver "$SOLVER" "$MODEL" "$INPUT_PATH"
  else
    echo "File not found: $INPUT_PATH"
  fi
done

echo "LA tests completed."
echo "Starting ORB tests..."
NUM_TESTS=10

for i in $(seq 1 $NUM_TESTS); do
  FILENAME=$(printf "orb%02d" "$i")
  FILENAMEDZN="${FILENAME}.dzn"
  INPUT_PATH="${INPUT_DIR}/${FILENAMEDZN}"
  OUTPUT_PATH="${OUTPUT_DIR}/output_${FILENAME}_ef.txt"

  if [[ -f "$INPUT_PATH" ]]; then
    echo "Running test: $FILENAME"
    minizinc -s -a -t 1200000 -o "$OUTPUT_PATH" --output-time --solver "$SOLVER" "$MODEL" "$INPUT_PATH"
  else
    echo "File not found: $INPUT_PATH"
  fi
done

echo "ORB tests completed."
echo "Starting ABZ tests..."

for i in $(seq 5 9); do
  FILENAME="abz${i}"
  FILENAMEDZN="${FILENAME}.dzn"
  INPUT_PATH="${INPUT_DIR}/${FILENAMEDZN}"
  OUTPUT_PATH="${OUTPUT_DIR}/output_${FILENAME}_ef.txt"

  if [[ -f "$INPUT_PATH" ]]; then
    echo "Running test: $FILENAME"
    minizinc -s -a -t 1200000 -o "$OUTPUT_PATH" --output-time --solver "$SOLVER" "$MODEL" "$INPUT_PATH"
  else
    echo "File not found: $INPUT_PATH"
  fi
done

echo "ABZ tests completed."
echo "Starting SWV tests..."

NUM_TESTS=20

for i in $(seq 1 $NUM_TESTS); do
  FILENAME=$(printf "swv%02d" "$i")
  FILENAMEDZN="${FILENAME}.dzn"
  INPUT_PATH="${INPUT_DIR}/${FILENAMEDZN}"
  OUTPUT_PATH="${OUTPUT_DIR}/output_${FILENAME}_ef.txt"

  if [[ -f "$INPUT_PATH" ]]; then
    echo "Running test: $FILENAME"
    minizinc -s -a -t 1200000 -o "$OUTPUT_PATH" --output-time --solver "$SOLVER" "$MODEL" "$INPUT_PATH"
  else
    echo "File not found: $INPUT_PATH"
  fi
done

echo "SWV tests completed."
echo "All tests completed."
