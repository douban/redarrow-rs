# redarrow-client

Redarrow bindings for Rust

Originally from https://github.intra.douban.com/platform/redarrow


## comparison with other clients

redarrow (python) with entrypoint
```shell
>_ sudo time -v /usr/bin/redarrow --host sa uptime
 01:17:00 up 110 days,  9:13,  4 users,  load average: 2.00, 2.92, 2.93
        Command being timed: "/usr/bin/redarrow --host sa uptime"
        User time (seconds): 0.60
        System time (seconds): 0.11
        Percent of CPU this job got: 95%
        Elapsed (wall clock) time (h:mm:ss or m:ss): 0:00.75
        Average shared text size (kbytes): 0
        Average unshared data size (kbytes): 0
        Average stack size (kbytes): 0
        Average total size (kbytes): 0
        Maximum resident set size (kbytes): 23400
        Average resident set size (kbytes): 0
        Major (requiring I/O) page faults: 0
        Minor (reclaiming a frame) page faults: 4384
        Voluntary context switches: 4
        Involuntary context switches: 144
        Swaps: 0
        File system inputs: 0
        File system outputs: 0
        Socket messages sent: 0
        Socket messages received: 0
        Signals delivered: 0
        Page size (bytes): 4096
        Exit status: 0
```

redarrow (python) as script
```shell
>_ sudo time -v /usr/bin/redarrow-cli --host sa uptime
 01:17:48 up 110 days,  9:14,  4 users,  load average: 2.12, 2.82, 2.89
        Command being timed: "/usr/bin/redarrow-cli --host sa uptime"
        User time (seconds): 0.07
        System time (seconds): 0.03
        Percent of CPU this job got: 70%
        Elapsed (wall clock) time (h:mm:ss or m:ss): 0:00.16
        Average shared text size (kbytes): 0
        Average unshared data size (kbytes): 0
        Average stack size (kbytes): 0
        Average total size (kbytes): 0
        Maximum resident set size (kbytes): 14428
        Average resident set size (kbytes): 0
        Major (requiring I/O) page faults: 0
        Minor (reclaiming a frame) page faults: 2297
        Voluntary context switches: 28
        Involuntary context switches: 11
        Swaps: 0
        File system inputs: 0
        File system outputs: 0
        Socket messages sent: 0
        Socket messages received: 0
        Signals delivered: 0
        Page size (bytes): 4096
        Exit status: 0
```

redarrow-client (golang) https://github.intra.douban.com/dae/redarrow-client
```shell
>_ sudo time -v go/bin/redarrow -host sa uptime
 01:18:56 up 110 days,  9:15,  4 users,  load average: 2.00, 2.67, 2.84
        Command being timed: "go/bin/redarrow -host sa uptime"
        User time (seconds): 0.00
        System time (seconds): 0.00
        Percent of CPU this job got: 33%
        Elapsed (wall clock) time (h:mm:ss or m:ss): 0:00.04
        Average shared text size (kbytes): 0
        Average unshared data size (kbytes): 0
        Average stack size (kbytes): 0
        Average total size (kbytes): 0
        Maximum resident set size (kbytes): 6952
        Average resident set size (kbytes): 0
        Major (requiring I/O) page faults: 0
        Minor (reclaiming a frame) page faults: 477
        Voluntary context switches: 55
        Involuntary context switches: 5
        Swaps: 0
        File system inputs: 0
        File system outputs: 0
        Socket messages sent: 0
        Socket messages received: 0
        Signals delivered: 0
        Page size (bytes): 4096
        Exit status: 0
```

redarrow-client (rust)
```shell
>_ sudo time -v /usr/bin/redarrow-client --host sa uptime
 01:21:06 up 110 days,  9:18,  4 users,  load average: 6.37, 3.78, 3.20
        Command being timed: "/usr/bin/redarrow-client --host sa uptime"
        User time (seconds): 0.00
        System time (seconds): 0.00
        Percent of CPU this job got: 15%
        Elapsed (wall clock) time (h:mm:ss or m:ss): 0:00.05
        Average shared text size (kbytes): 0
        Average unshared data size (kbytes): 0
        Average stack size (kbytes): 0
        Average total size (kbytes): 0
        Maximum resident set size (kbytes): 6588
        Average resident set size (kbytes): 0
        Major (requiring I/O) page faults: 0
        Minor (reclaiming a frame) page faults: 308
        Voluntary context switches: 8
        Involuntary context switches: 1
        Swaps: 0
        File system inputs: 0
        File system outputs: 0
        Socket messages sent: 0
        Socket messages received: 0
        Signals delivered: 0
        Page size (bytes): 4096
        Exit status: 0
```
