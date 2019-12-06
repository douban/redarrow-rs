# redarrow-client

Redarrow bindings for Rust

Originally from https://github.intra.douban.com/platform/redarrow


## comparison with other clients

redarrow (python) with entrypoint
```shell
>_ sudo time /usr/bin/redarrow --host sa uptime
 01:34:58 up 110 days,  9:31,  4 users,  load average: 1.36, 1.79, 2.25
0.58user 0.14system 0:00.75elapsed 96%CPU (0avgtext+0avgdata 23376maxresident)k
0inputs+0outputs (0major+4400minor)pagefaults 0swaps
```

redarrow (python) as script
```shell
>_ sudo time /usr/bin/redarrow-cli --host sa uptime
 01:35:38 up 110 days,  9:32,  4 users,  load average: 1.58, 1.81, 2.24
0.08user 0.01system 0:00.14elapsed 69%CPU (0avgtext+0avgdata 14476maxresident)k
0inputs+0outputs (0major+2300minor)pagefaults 0swaps
```

redarrow-client (golang) https://github.intra.douban.com/dae/redarrow-client
```shell
>_ sudo time go/bin/redarrow -host sa uptime
 01:36:08 up 110 days,  9:33,  4 users,  load average: 1.60, 1.79, 2.21
0.00user 0.00system 0:00.03elapsed 30%CPU (0avgtext+0avgdata 6884maxresident)k
0inputs+0outputs (0major+485minor)pagefaults 0swaps
```

redarrow-client (rust)
```shell
>_ sudo time /usr/bin/redarrow-client --host sa uptime
 01:36:29 up 110 days,  9:33,  4 users,  load average: 1.30, 1.71, 2.18
0.00user 0.00system 0:00.03elapsed 18%CPU (0avgtext+0avgdata 6736maxresident)k
0inputs+0outputs (0major+314minor)pagefaults 0swaps
```
