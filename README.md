# taiko-batch-analyzer

Get L2 block info:

```
➜  taiko-batch-analyzer git:(main) ✗ L1_RPC_URL=https://eth.merkle.io L2_RPC_URL=https://rpc.mainnet.taiko.xyz TAIKO_INBOX_ADDRESS=0x06a9Ab27c7e2255df1815E6CC0168d7755Feb19a cargo run -- --l1-block 23118780
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.43s
     Running `target/debug/taiko-batch-analyzer --l1-block 23118780`
Scanning for BatchProposed events:
  Contract: 0x06a9Ab27c7e2255df1815E6CC0168d7755Feb19a
  L1 blocks: 23118780 to 23118780
  Total logs from contract in block range: 14
Event #0: batch_id=1320758 blocks 1323035..=1323043 (9 blocks)
  L2 block 1323035: 2 txs
  L2 block 1323036: 2 txs
  L2 block 1323037: 2 txs
  L2 block 1323038: 2 txs
  L2 block 1323039: 3 txs
  L2 block 1323040: 3 txs
  L2 block 1323041: 2 txs
  L2 block 1323042: 3 txs
  L2 block 1323043: 1 txs
  total txs: 20
  total paid: 199618893553900 wei (~0.000199618893553900 ETH)
```

Example with public RPCs. They might rate-limit you if you run on larger batches.
