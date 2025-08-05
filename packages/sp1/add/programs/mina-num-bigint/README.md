```
 make hash MODE=execute ITERATIONS=10 TYPE=mina
Running hash operation...
Mode: execute
Type: mina
Iterations: 10
⏱️  Start time: 2025-08-05 10:20:33
Executing mina hash program...
   Compiling add-script v0.1.0 (/Users/mike/Documents/Silvana/zk-tests/packages/sp1/add/script)
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: add-program built at 2025-08-05 10:20:36
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: aggregate-program built at 2025-08-05 10:20:37
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: sha256-program built at 2025-08-05 10:20:39
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: p3-program built at 2025-08-05 10:20:40
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: mina-program built at 2025-08-05 10:20:43
    Finished `release` profile [optimized] target(s) in 14.03s
     Running `/Users/mike/Documents/Silvana/zk-tests/packages/sp1/add/target/release/hash --execute --type mina --iterations 10`
Hash type: Mina
Iterations: 10
Input data: [1, 2, 3]
2025-08-05T07:20:50.467573Z  INFO execute: clk = 0 pc = 0x2110c8
2025-08-05T07:20:50.683624Z  INFO execute: clk = 10000000 pc = 0x226ca0
2025-08-05T07:20:50.897283Z  INFO execute: clk = 20000000 pc = 0x21d1a8
2025-08-05T07:20:51.108518Z  INFO execute: clk = 30000000 pc = 0x226ca0
2025-08-05T07:20:51.322848Z  INFO execute: clk = 40000000 pc = 0x21d1d0
2025-08-05T07:20:51.533989Z  INFO execute: clk = 50000000 pc = 0x211600
2025-08-05T07:20:51.749256Z  INFO execute: clk = 60000000 pc = 0x210c50
2025-08-05T07:20:51.960613Z  INFO execute: clk = 70000000 pc = 0x226c94
2025-08-05T07:20:52.174131Z  INFO execute: clk = 80000000 pc = 0x2186c4
2025-08-05T07:20:52.387079Z  INFO execute: clk = 90000000 pc = 0x21cfc4
2025-08-05T07:20:52.616452Z  INFO execute: clk = 100000000 pc = 0x226cb8
2025-08-05T07:20:52.829601Z  INFO execute: clk = 110000000 pc = 0x226990
2025-08-05T07:20:53.042709Z  INFO execute: clk = 120000000 pc = 0x226c94
2025-08-05T07:20:53.257966Z  INFO execute: clk = 130000000 pc = 0x2184ac
2025-08-05T07:20:53.471816Z  INFO execute: clk = 140000000 pc = 0x211588
2025-08-05T07:20:53.688756Z  INFO execute: clk = 150000000 pc = 0x21dff4
2025-08-05T07:20:53.902776Z  INFO execute: clk = 160000000 pc = 0x226c90
2025-08-05T07:20:54.117696Z  INFO execute: clk = 170000000 pc = 0x21d1b4
2025-08-05T07:20:54.333363Z  INFO execute: clk = 180000000 pc = 0x21d430
2025-08-05T07:20:54.428301Z  INFO execute: gas: 180592451
2025-08-05T07:20:54.429565Z  INFO execute: close time.busy=3.97s time.idle=2.08µs
Program executed successfully.
Hash digest: 0x366e46102b0976735ed1cc8820c7305822a448893fee8ceeb42a3012a4663fd0
Number of cycles: 184025615
Number of syscalls: 22
Total instructions: 184025637

=== Detailed Execution Report ===
gas: 180592451
opcode counts (184025615 total instructions):
    47685699 add
    17560604 sub
    17312551 lw
    15575968 srl
    14308101 sltu
    13255179 sw
     8903475 or
     8332026 sll
     6991460 bne
     6222959 beq
     4709594 blt
     3917776 mul
     3531551 mulhu
     3408777 jal
     2341591 bgeu
     2232535 jalr
     1908252 and
     1583083 bltu
     1382872 xor
     1116281 auipc
      722793 lbu
      606034 sb
      178142 divu
      157020 sra
       69340 bge
       10040 lb
        1770 remu
          60 slt
          60 sh
          22 ecall
syscall counts (22 total syscall instructions):
    8 commit
    8 commit_deferred_proofs
    2 hint_len
    2 hint_read
    1 halt
    1 write

⏱️  End time: 2025-08-05 10:20:54
⏱️  Duration: 21 seconds


add % make hash MODE=prove ITERATIONS=1 TYPE=mina
Running hash operation...
Mode: prove
Type: mina
Iterations: 1
⏱️  Start time: 2025-08-05 10:22:47
Generating proof for mina hash program...
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: add-program built at 2025-08-05 10:20:36
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: aggregate-program built at 2025-08-05 10:20:37
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: sha256-program built at 2025-08-05 10:20:39
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: p3-program built at 2025-08-05 10:20:40
warning: add-script@0.1.0: rustc +succinct --version: "rustc 1.88.0-dev\n"
warning: add-script@0.1.0: mina-program built at 2025-08-05 10:20:43
    Finished `release` profile [optimized] target(s) in 0.69s
     Running `/Users/mike/Documents/Silvana/zk-tests/packages/sp1/add/target/release/hash --prove --type mina --iterations 1`
Hash type: Mina
Iterations: 1
Input data: [1, 2, 3]
Generating proof...
2025-08-05T07:22:51.838183Z  INFO prove_core: clk = 0 pc = 0x2110c8
2025-08-05T07:22:51.851725Z  INFO prove_core: clk = 0 pc = 0x2110c8
2025-08-05T07:22:52.023412Z  INFO prove_core:generate main traces: close time.busy=88.7ms time.idle=1.13µs index=0
2025-08-05T07:22:52.108095Z  INFO prove_core: clk = 10000000 pc = 0x226ca0
2025-08-05T07:22:52.248074Z  INFO prove_core:generate main traces: close time.busy=67.8ms time.idle=959ns index=1
2025-08-05T07:22:52.429627Z  INFO prove_core:generate main traces: close time.busy=71.9ms time.idle=1.25µs index=2
2025-08-05T07:23:01.315924Z  INFO prove_core:generate main traces: close time.busy=60.1ms time.idle=1.25µs index=3
2025-08-05T07:23:09.283692Z  INFO prove_core:generate main traces: close time.busy=66.0ms time.idle=1.21µs index=4
2025-08-05T07:23:17.227979Z  INFO prove_core:generate main traces: close time.busy=54.0ms time.idle=1.33µs index=5
2025-08-05T07:23:25.298494Z  INFO prove_core:generate main traces: close time.busy=53.4ms time.idle=1.63µs index=6
2025-08-05T07:23:33.222091Z  INFO prove_core:generate main traces: close time.busy=54.4ms time.idle=1.54µs index=7
2025-08-05T07:23:41.057526Z  INFO prove_core:generate main traces: close time.busy=57.0ms time.idle=1.08µs index=8
2025-08-05T07:23:49.079582Z  INFO prove_core:generate main traces: close time.busy=59.3ms time.idle=1.79µs index=9
2025-08-05T07:23:57.359561Z  INFO prove_core:generate main traces: close time.busy=57.0ms time.idle=1.62µs index=10
2025-08-05T07:24:05.585798Z  INFO prove_core:generate main traces: close time.busy=55.2ms time.idle=1.88µs index=11
2025-08-05T07:24:13.599099Z  INFO prove_core:generate main traces: close time.busy=64.7ms time.idle=1.25µs index=12
2025-08-05T07:24:21.492244Z  INFO prove_core:generate main traces: close time.busy=53.0ms time.idle=1.42µs index=13
2025-08-05T07:24:29.422722Z  INFO prove_core:generate main traces: close time.busy=55.9ms time.idle=1.54µs index=14
2025-08-05T07:24:37.341496Z  INFO prove_core:generate main traces: close time.busy=55.6ms time.idle=1.71µs index=15
2025-08-05T07:24:45.167772Z  INFO prove_core:generate main traces: close time.busy=61.3ms time.idle=1.42µs index=16
2025-08-05T07:24:53.064303Z  INFO prove_core:generate main traces: close time.busy=56.9ms time.idle=1.46µs index=17
2025-08-05T07:25:01.232458Z  INFO prove_core:generate main traces: close time.busy=53.4ms time.idle=4.37µs index=18
2025-08-05T07:25:09.291438Z  INFO prove_core: clk = 10000000 pc = 0x226ca0
2025-08-05T07:25:09.452765Z  INFO prove_core:generate main traces: close time.busy=65.1ms time.idle=1.21µs index=19
2025-08-05T07:25:17.322822Z  INFO prove_core:generate main traces: close time.busy=53.4ms time.idle=1.50µs index=20
2025-08-05T07:25:24.870614Z  INFO prove_core:generate main traces: close time.busy=57.2ms time.idle=1.21µs index=21
2025-08-05T07:25:32.700899Z  INFO prove_core:generate main traces: close time.busy=49.3ms time.idle=1.12µs index=22
2025-08-05T07:25:40.508332Z  INFO prove_core:generate main traces: close time.busy=51.4ms time.idle=2.25µs index=23
2025-08-05T07:25:48.407869Z  INFO prove_core:generate main traces: close time.busy=51.3ms time.idle=1.46µs index=24
2025-08-05T07:25:56.444582Z  INFO prove_core:generate main traces: close time.busy=54.0ms time.idle=1.08µs index=25
2025-08-05T07:26:04.378401Z  INFO prove_core:generate main traces: close time.busy=52.7ms time.idle=1.83µs index=26
2025-08-05T07:26:12.377753Z  INFO prove_core:generate main traces: close time.busy=53.7ms time.idle=1.33µs index=27
2025-08-05T07:26:20.097029Z  INFO prove_core:generate main traces: close time.busy=56.5ms time.idle=1.38µs index=28
2025-08-05T07:26:27.791946Z  INFO prove_core:generate main traces: close time.busy=48.7ms time.idle=958ns index=29
2025-08-05T07:26:35.438169Z  INFO prove_core:generate main traces: close time.busy=49.8ms time.idle=1.54µs index=30
2025-08-05T07:26:43.060565Z  INFO prove_core:generate main traces: close time.busy=53.1ms time.idle=1.25µs index=31
2025-08-05T07:26:50.709950Z  INFO prove_core:generate main traces: close time.busy=53.4ms time.idle=1.04µs index=32
2025-08-05T07:26:58.338995Z  INFO prove_core:generate main traces: close time.busy=57.6ms time.idle=1.83µs index=33
2025-08-05T07:27:06.009712Z  INFO prove_core:generate main traces: close time.busy=55.4ms time.idle=1.25µs index=34
2025-08-05T07:27:14.366560Z  INFO prove_core:generate main traces: close time.busy=596ms time.idle=1.54µs index=35
2025-08-05T07:27:40.912570Z  INFO prove_core: close time.busy=289s time.idle=188ms
Proof generated successfully!
Proving time: 289.08s
Average time per iteration: 289082.00ms
2025-08-05T07:27:45.544101Z  INFO verify: close time.busy=4.60s time.idle=1.75µs

=== Verification ===
Proof verified successfully!
Verification time: 4.65s

=== Output ===
Hash digest: 0x366e46102b0976735ed1cc8820c7305822a448893fee8ceeb42a3012a4663fd0
⏱️  End time: 2025-08-05 10:27:45
⏱️  Duration: 298 seconds
```
