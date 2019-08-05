

# Map processes to applications with sonar map

Map processes to applications:

```
$ sonar map --input-dir /home/user/snap-outputs
```


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


## How to use your own mapping files

Sonar uses the following mapping files: https://github.com/nordichpc/sonar/tree/master/sonar/mapping

The mapping files (`string_map.txt` and `regex_map.txt`) contain a space-separated
(does not matter how many spaces) mapping from process to application.

You can use your own mapping files instead:

```
$ sonar map --input-dir /home/user/snap-outputs \
            --str-map-file /home/user/my-own-mapping/string_map.txt \
            --re-map-file /home/user/my-own-mapping/regex_map.txt
```


You are welcome to use your own but encouraged to contribute mappings to https://github.com/nordichpc/sonar/tree/master/sonar/mapping.


## Exporting CSV data

You can also export daily, weekly, and monthly CPU load percentages in CSV format for further postprocessing, e.g.
using https://github.com/NordicHPC/sonar-web.

Example daily sums:
```
$ sonar map --input-dir /home/user/snap-outputs \
            --export-csv daily

date,VASP,Gaussian,Qdyn,LAMMPS,GPAW,TEXAS,StagYY,ROMS,ParaDiS,unknown
2019-06-23,50.87,7.74,0.99,14.01,0.00,0.00,1.73,3.45,3.25,6.25
2019-06-24,41.90,14.39,0.96,16.70,0.00,0.00,1.85,3.48,2.72,6.64
2019-06-25,43.61,17.83,4.73,6.01,0.00,0.00,1.83,1.90,1.63,12.79
2019-06-26,39.81,23.33,0.00,0.00,0.00,0.00,2.42,0.00,4.70,14.94
2019-06-27,32.17,23.50,1.28,0.00,0.00,0.00,3.70,0.00,16.59,7.84
2019-06-28,24.31,16.53,4.83,0.00,0.00,22.00,4.67,2.26,14.20,3.58
2019-06-29,22.05,18.36,0.00,0.48,0.00,45.31,3.31,0.00,3.65,1.70
2019-06-30,22.90,9.74,0.00,2.72,0.00,55.44,0.87,0.00,0.00,3.62
2019-07-01,25.39,8.68,0.00,8.08,0.00,52.28,0.83,0.00,0.00,0.63
```

Example weekly sums:
```
$ sonar map --input-dir /home/user/snap-outputs \
            --export-csv weekly --num-days 200

week,VASP,Gaussian,Qdyn,LAMMPS,GPAW,TEXAS,StagYY,ROMS,ParaDiS,unknown
2019-19,21.81,22.91,0.96,3.43,33.50,0.64,2.73,3.17,0.24,1.28
2019-20,17.40,25.72,17.52,2.94,15.89,0.38,4.59,0.74,0.00,4.83
2019-21,19.95,33.73,11.26,10.10,3.00,0.17,4.24,4.09,0.00,5.29
2019-22,21.02,22.36,14.46,9.29,5.63,0.00,3.61,4.47,0.28,10.40
2019-23,18.94,13.74,20.22,6.89,13.63,0.00,3.62,5.20,1.76,3.44
2019-24,26.37,8.64,20.59,14.48,0.34,0.00,2.33,0.48,6.40,5.88
2019-25,33.99,13.86,15.97,12.57,0.00,0.00,1.50,2.01,0.64,9.33
2019-26,32.32,17.64,1.71,3.68,0.00,17.79,2.67,1.09,6.17,7.26
2019-27,25.39,8.68,0.00,8.08,0.00,52.28,0.83,0.00,0.00,0.63
```
