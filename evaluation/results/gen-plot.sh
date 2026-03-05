results=${1-"csv/results.csv"}
PREPROCESSED="csv/results-preprocessed.csv"
CUMULATIVE_TIME="csv/thesy-cumulative-time.csv"
CUMULATIVE_MEMORY="csv/thesy-cumulative-memory.csv"

echo "PREPROCESSING $results INTO $PREPROCESSED"
python3.12 preprocess.py $results $PREPROCESSED
echo "DONE."

echo "PLOTTING CUMULATIVE TIME $PREPROCESSED INTO $CUMULATIVE_TIME"
python3.12 gen-plot.py $PREPROCESSED time > $CUMULATIVE_TIME
echo "DONE."

echo "PLOTTING CUMULATIVE MEMORY $PREPROCESSED INTO $CUMULATIVE_MEMORY"
python3.12 gen-plot.py $PREPROCESSED memory > $CUMULATIVE_MEMORY
echo "DONE."