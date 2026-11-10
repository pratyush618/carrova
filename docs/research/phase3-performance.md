# Phase 3 — transfer performance research (2026-11-08)

[M] = measured on the dev laptop (Ryzen 5 5600H, AES-NI), [S] = from source/docs. Bench sources: `/tmp/carrova-bench/qb` (throwaway).

## Network (quinn 0.11 / rustls 0.23 ring)

| Setting | Value | Why |
|---|---|---|
| `stream_receive_window` | 4 MiB | default 1.25 MB tight for Wi-Fi bursts / 10 GbE |
| `receive_window` | 32 MiB | default unlimited; bounds receiver memory |
| `send_window` | 32 MiB | default 10 MB; keep all streams fed |
| `max_concurrent_bidi_streams` | 16 (sender) | explicit bound; we use 8 |
| `initial_rtt` | 10 ms | default 333 ms → slow early loss recovery on LAN |
| congestion | Cubic (default) | quinn BBR is marked experimental |
| MTU discovery | default on | measured path MTU 1452 [M] |
| GSO/GRO, recvmmsg | default on | quinn-udp enables on Linux [S] |
| idle / keep-alive | 30 s / 5 s | default has no keep-alive → Wi-Fi roam/pause kills session |
| UDP socket buffers | socket2: request 8 MiB SND/RCV, read back, warn if capped | quinn never sets them; unprivileged clamp to `rmem_max` (Ubuntu default 212 992 B) [S] |
| cipher | ring default order (AES-128-GCM first) | AES-GCM 48.5 vs ChaCha 17.7 Gbit/s [M]; e2e 5.43 vs 4.41 Gbit/s [M]; quinn needs AES-128-GCM for Initial |
| connections | 1 connection, 8 streams (1/2.5 GbE) | 1 vs 8 vs 32 streams: 5.46/5.43/5.18 Gbit/s loopback [M]; 2–4 connections only worth it for 10 GbE |

Pitfalls (must avoid):
1. CBOR `Vec<u8>` without `serde_bytes` encodes as integer array: 2.07 vs 29 Gbit/s [M]. Chunk frames use a fixed binary header (`u32 len | 32 B hash`) + raw bytes; CBOR only for control messages, with `serde_bytes` for any byte fields.
2. Request/response stalls: keep ≥2 batches in flight per stream (~4 MiB outstanding/stream); coalesce tiny chunks into ≤1 MiB frames (BLAKE3 on 1 KiB: 9.1 vs 36 Gbit/s at 64 KiB [M]).
3. No sync work on tokio workers: disk reads + BLAKE3 + engine callback on blocking threads; bounded channel (~64 MiB) between transport and engine for backpressure.
4. Socket buffer silently capped: read back, log, document `sysctl net.core.rmem_max=7500000`. Dev laptop has 4 MiB, so CI e2e must also run with the Ubuntu default.
5. Default `initial_rtt` + no keep-alive on Wi-Fi.

Other: BLAKE3 36 Gbit/s single-thread [M] — no rayon needed per chunk. mdns-sd resolve ~50–200 ms wired, 1–2 s lossy Wi-Fi [S]; browse in background, fallback ticket.

Expected: loopback ≈5–5.6 Gbit/s, CPU-bound [M]; 1 GbE ≈ wire speed (≈940 Mbit/s goodput, extrapolated); small-file workloads bound by receiver batching/fsync, not network.

## Disk path

Dev laptop: ext4 on QLC NVMe, 92 % full — 370–930 MB/s until the SLC cache (~256 MiB) fills, then 11–30 MB/s sustained [M]. Bytes written per payload byte dominate wall time.

Phase 2 path (from code): ~3 bytes written + ~4 read per payload byte (chunk files → assemble → apply temp copy); 6 BLAKE3 + 2 FastCDC passes (~2.1 s CPU/GB); ~5 000 fsyncs per 1000 created files, ~8 000 per 1000 replaced.

Measured [M]:
- per-chunk temp+fsync+rename: 64 KiB → 6.2 MB/s; 4 KiB → 1 245 chunks/s (1 GbE needs ~1 700/s) → rejected
- 4096 tiny files: fsync each 1 428/s, batched fdatasync 1 465/s, syncfs 3 099/s, append pack 7 306/s
- apply 1000 × 2 KiB: current 6.2 s create / 3.9 s replace; batched journal + copy 0.68/0.71 s; batched journal + rename staged file into place 0.16/0.32 s
- sender reopen-per-chunk nofollow: 98k chunks/s (43k with BLAKE3) vs fd cache 248k/61k → reopen costs <4 % of a core at 1 GbE; keep it

Decision — write directly into final staged files, then rename into place:
1. `StageLayout` from approved plan + manifest: staged file n → [(offset, ChunkRef)]; hash → all (file, offset) (free dedup). `staging/files/<n>` (600) pre-sized with `set_len` → early ENOSPC → disk-full pause.
2. Receive: chunk hash verified once on arrival, bytes written at every known offset (positional write).
3. Checkpoint every 64 MiB / 2 s / on pause/close: fdatasync dirty files (or `Platform::sync_fs`), fsync `files/`, then append verified hashes to `staging/progress.log` (CRC-framed, torn tail dropped) + fdatasync. A chunk counts as verified only once its record is durable → crash leaves record-or-refetch.
4. File complete: size + whole-file BLAKE3 vs manifest.
5. Apply: batched journal intents; rename staged file into place (`rename_noreplace`), mode/mtime set on the staged file first, one dir fsync per distinct dir; EXDEV → current copy path.
6. Hashing: keep per-chunk on receive (untrusted peer) + whole-file after staging; drop assemble re-hash and apply `hash_file` (bytes never leave the 0700 state dir).
7. Space: `bytes + 5 % + 64 MiB` when state dir shares the target filesystem (`Platform::fs_id`), per-filesystem otherwise.

Result: 1× bytes written (2× on EXDEV), ~0.5 s CPU/GB hashing, ~1 fsync per 64 MiB in staging, ~2 journal syncs + distinct dirs per apply batch. Network-limited wherever the disk sustains it.
