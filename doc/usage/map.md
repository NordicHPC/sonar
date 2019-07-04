

# Map processes to applications with sonar map

Map processes to applications:

```
$ sonar map --input-dir /home/user/snap-outputs \
            --str-map-file example-mapping/string_map.txt \
            --re-map-file example-mapping/regex_map.txt
```

The mapping files (`string_map.txt` and `regex_map.txt`) contain a space-separated
(does not matter how many spaces) mapping from process to application.
Example mapping files: https://github.com/nordichpc/sonar/tree/master/example-mapping

You are welcome to use your own but encouraged to contribute mappings to our example files.


## Example output

In this example the top users have been redacted:

```
sonar v0.1.0
summary generated on 2019-05-21 16:58:23.457775
percentage cutoff: 0.5%

  app              top users              use  reserve
======================================================

- Gaussian                              26.17%  31.35%
                   user                 15.48%  16.83% (16-160 cores, 1875 MB)
                   user                  1.25%   2.56% (64-160 cores, 2000-6400 MB)

- VASP                                  19.07%  16.35%
                   user                  6.71%   4.80% (64-80 cores, 2000 MB)
                   user                  3.14%   3.59% (16-160 cores, 2000 MB)

- GPAW                                  19.38%  12.30%
                   user                 18.82%  11.92% (40 cores, 1523 MB)
                   user                  0.31%   0.22% (32 cores, 31000 MB)

- Qdyn                                  11.49%   9.16%
                   user                 11.16%   8.94% (16 cores, 1523 MB)
                   user                  0.21%   0.14% (20 cores, 1024 MB)

- StagYY                                 4.11%   7.61%
                   user                  2.00%   3.84% (32-64 cores, 4000 MB)
                   user                  2.10%   3.77% (64 cores, 4000 MB)

- GROMACS                                3.49%   3.30%
                   user                  3.41%   2.50% (32 cores, 1523 MB)
                   user                  0.08%   0.80% (3-6 cores, 1523 MB)

- NAMD                                   1.02%   2.32%
                   user                  1.02%   2.32% (256 cores, 500 MB)

- LAMMPS                                 3.45%   2.21%
                   user                  2.85%   1.81% (32-128 cores, 1523 MB)
                   user                  0.53%   0.34% (80 cores, 1523 MB)

- Orca                                   1.88%   1.37%
                   user                  1.67%   1.17% (32-64 cores, 96000 MB)
                   user                  0.20%   0.20% (16 cores, 96000 MB)

- ISSM                                   1.58%   1.10%
                   user                  0.78%   0.54% (16 cores, 1500-1600 MB)
                   user                  0.69%   0.48% (16 cores, 1600 MB)

- ROMS                                   1.21%   0.95%
                   user                  1.19%   0.93% (320 cores, 1600 MB)
                   user                  0.01%   0.01% (16-320 cores, 1600 MB)

- TURBOMOLE                              0.06%   0.83%
                   user                  0.06%   0.83% (1-20 cores, 1500 MB)

- Python script                          0.12%   0.61%
                   user                  0.08%   0.43% (20-50 cores, 5000 MB)
                   user                  0.00%   0.05% (1-8 cores, 2000 MB)

  unmapped                               4.71%   7.94%
------------------------------------------------------

- lmp                                    2.08%   3.09%
                   user                  2.08%   3.09% (1 cores, 1523 MB)

- pmi_proxy                              0.09%   1.14%
                   user                  0.09%   1.07% (16-160 cores, 2000 MB)
                   user                  0.00%   0.04% (20 cores, 1523 MB)
```


## Exporting CSV data

You can also export daily CPU load percentages in CSV format for further postprocessing, e.g.
using https://github.com/NordicHPC/sonar-web.

```
$ sonar map --input-dir /home/user/snap-outputs \
            --str-map-file example-mapping/string_map.txt \
            --re-map-file example-mapping/regex_map.txt \
            --export-csv-daily

date,Gaussian,GPAW,VASP,Qdyn,StagYY,GROMACS,LAMMPS,Orca,ISSM,unknown
2019-05-10,20.54,30.10,29.41,0.03,2.15,3.86,2.09,2.05,1.46,1.21
2019-05-11,17.38,35.81,22.40,2.07,2.43,2.81,2.95,1.87,1.01,2.45
2019-05-12,29.15,32.05,19.21,0.08,3.20,1.40,4.27,2.11,0.31,1.56
2019-05-13,22.17,38.65,15.47,1.54,5.39,2.21,3.46,1.73,0.95,2.06
2019-05-14,28.38,29.46,19.06,3.01,4.37,1.54,2.73,0.95,1.00,5.48
2019-05-15,31.28,16.45,14.84,18.86,1.19,0.41,2.06,0.74,0.92,12.38
2019-05-16,32.54,10.80,15.14,27.77,1.13,0.76,3.01,0.46,2.58,4.23
2019-05-17,21.89,0.49,16.08,47.72,2.10,1.17,1.86,0.47,4.34,0.83
2019-05-18,19.72,0.00,16.11,27.23,8.41,11.29,1.96,1.04,3.69,4.81
2019-05-19,23.12,8.28,25.91,0.76,10.41,9.20,5.63,2.37,0.91,6.34
2019-05-20,29.28,18.00,22.43,0.24,5.15,4.20,6.65,5.49,0.83,4.17
2019-05-21,40.74,2.11,21.85,2.45,2.68,6.52,4.61,4.96,1.48,11.59
```
