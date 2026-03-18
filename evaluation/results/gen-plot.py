def argnext(type, default): 
    import sys
    return type(sys.argv.pop(0)) if len(sys.argv) > 0 else default

def main():
    import pandas as pd

    _PROGRAM = argnext(str, "gen-plot.py")
    INPUT_FILE = argnext(str, "./results.csv")
    PLOT = argnext(str, "time")        # two values: memory or time

    skipped = 0
    benchmark = []

    cloning_code = []
    cloning_secs = []
    cloning_gb = []
    cloning_branches = []

    versioned_code = []
    versioned_secs = []
    versioned_gb = []
    versioned_branches = []

    persistent_code = []
    persistent_secs = []
    persistent_gb = []
    persistent_branches = []

    colored_code = []
    colored_secs = []
    colored_gb = []
    colored_branches = []

    for line in open(INPUT_FILE).readlines()[1:]:
        line = line[:-1]
        if len(line) == 0: continue

        [
            b, 
            cr, ct, cm, cb, 
            vr, vt, vm, vb, 
            pr, pt, pm, pb, 
            er, et, em, eb
        ] = line.split(",")
        try:
            cr = int(cr)
            ct = float(ct)
            cm = float(cm)
            cb = float(cb)
            vr = int(vr)
            vt = float(vt)
            vm = float(vm)
            vb = float(vb)
            pr = int(pr)
            pt = float(pt)
            pm = float(pm)
            pb = float(pb)
            er = int(er)
            et = float(et)
            em = float(em)
            eb = float(eb)
        except ValueError as e:
            skipped += 1
            continue

        benchmark.append(b)
        cloning_code.append(cr)
        cloning_secs.append(ct)
        cloning_gb.append(cm)
        cloning_branches.append(cb)
        versioned_code.append(vr)
        versioned_secs.append(vt)
        versioned_gb.append(vm)
        versioned_branches.append(vb)
        persistent_code.append(pr)
        persistent_secs.append(pt)
        persistent_gb.append(pm)
        persistent_branches.append(pb)
        colored_code.append(er)
        colored_secs.append(et)
        colored_gb.append(em)
        colored_branches.append(eb)

    def cumulative(*datasets, sample_count=100, valueMapper=lambda x: x):
        m = valueMapper(min([min(dataset) for dataset in datasets]))
        M = valueMapper(max([max(dataset) for dataset in datasets]))
        print("0.0," + ",".join(["0"] * len(datasets)))
        for i in range(sample_count+1):
            threshold = m + (M-m) * i * 1.0 / sample_count
            counts = list(map(lambda dataset: len(list(filter(lambda d: valueMapper(d) <= threshold, dataset))), datasets))
            columns = [threshold] + counts
            print(",".join(map(str, columns)))

    if PLOT == "time":
        print("threshold,cloningTime,versionedTime,persistentTime,coloredTime")
        cumulative(cloning_secs, versioned_secs, persistent_secs, colored_secs, sample_count=1000)
    if PLOT == "memory":
        print("threshold,cloningMemory,versionedMemory,persistentMemory,coloredMemory")
        cumulative(cloning_gb, versioned_gb, persistent_gb, colored_gb, sample_count=1000, valueMapper=lambda x: round(x, 2))

if __name__ == "__main__":
    main()