#!/bin/bash

### 1. Description
### Generate all plots from the results in the csv folder.
### 2. Usage
### - ./build.sh

BUILD_ORDER=(
    cumulative-time
    scatter-time-cloning
    scatter-time-persistent
    scatter-time-easteregg
    cumulative-memory
    scatter-memory-cloning
    scatter-memory-persistent
    scatter-memory-easteregg
    thesy
)

mkdir -p build
for file in "${BUILD_ORDER[@]}"; do
    pdflatex -interaction=nonstopmode -output-directory=build "${file}.tex"
done

mv build/thesy.pdf ../pdf/results.pdf
rm -r *.aux *.log *.toc *.fdb_latex *.fdb_latexmk *.fls *.gz *.out *synctex* thesy.pdf
rm -r build