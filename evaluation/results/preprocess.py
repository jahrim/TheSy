def argnext(type, default): 
    import sys
    return type(sys.argv.pop(0)) if len(sys.argv) > 0 else default

def main():
    import os
    import pandas as pd
    try:
        _PROGRAM = argnext(str, "preprocess.py")
        INPUT_FILE = argnext(str, "./results.csv")
        dir_name, base_name = os.path.split(INPUT_FILE)
        OUTPUT_FILE = argnext(str, os.path.join(dir_name, "preprocessed-" + base_name))
        TIMEOUT = argnext(int, None)
        if TIMEOUT is None: 
            TIMEOUT = 300
            print(f"[Warning] No TIMEOUT provided, using default {TIMEOUT}s.")
        MAX_MEMORY = argnext(int, None)
        if MAX_MEMORY is None:
            MAX_MEMORY = 32
            print(f"[Warning] No MAX_MEMORY provided, using default {MAX_MEMORY}GB.")

    except Exception as e:
        print((
            "USAGE\n"
            "python3 preprocess.py input.csv [output.csv] [max_time] [max_memory]\n"
            "DESCRIPTION\n"
            "Preprocess the results of the evaluation:\n"
            "1. Replace every timeout with maximum time and memory X.\n"
            "2. Replace every out-of-memory with a set sentinel value Y > X.\n"
            "3. Replace every error with a set sentinel value Z > Y.\n"
        ))
        print(e)
        exit(1)

    print(f"Reading .csv from {INPUT_FILE}...")
    table = pd.read_csv(INPUT_FILE, keep_default_na=False)

    print("Detecting variants...", end=' ')
    variants = [col[:-5] for col in table.columns if col.endswith('_code')]
    print(f"{variants}")

    print("Evaluating sentinels...")
    max_memory_gb = MAX_MEMORY
    max_time_secs = TIMEOUT
    oom_memory_sentinel = max_memory_gb * 1.05
    oom_time_sentinel = max_time_secs * 1.05
    error_memory_sentinel = max_memory_gb * 1.1
    error_time_sentinel = max_time_secs * 1.1
    print(f"max_memory={max_memory_gb}GB, max_time={max_time_secs}s")
    print(f"oom_memory_sentinel={oom_memory_sentinel}GB, oom_time_sentinel={oom_time_sentinel}s")
    print(f"error_memory_sentinel={error_memory_sentinel}GB, error_time_sentinel={error_time_sentinel}s")

    print("Applying sentinels...", end=' ')
    for variant in variants:
        code_col = f'{variant}_code'
        secs_col = f'{variant}_secs'
        kb_col = f'{variant}_kb'
        gb_col = f'{variant}_gb'

        if code_col in table.columns and secs_col in table.columns and kb_col in table.columns:
            table[kb_col] = table[kb_col].astype(float) / (1024 * 1024)
            table.rename(columns={kb_col: gb_col}, inplace=True)
            
            table.loc[table[gb_col] > max_memory_gb, code_col] = 134
            table.loc[table[secs_col] > max_time_secs, code_col] = 124
        
            # Errors
            table.loc[table[code_col] != 0, secs_col] = error_time_sentinel
            table.loc[table[code_col] != 0, gb_col] = error_memory_sentinel
            # Timeouts
            table.loc[table[code_col] == 124, secs_col] = max_time_secs
            table.loc[table[code_col] == 124, gb_col] = oom_memory_sentinel
            # OOM
            table.loc[table[code_col] == 134, secs_col] = oom_time_sentinel
            table.loc[table[code_col] == 134, gb_col] = max_memory_gb
    print("Done.")

    print(f"Saving processed .csv to {OUTPUT_FILE}...", end=' ')
    table.to_csv(OUTPUT_FILE, index=False)
    print(f"Done.")

if __name__ == "__main__":
    main()