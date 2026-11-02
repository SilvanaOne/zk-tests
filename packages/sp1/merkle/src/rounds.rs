//! The measurements, as explicit lists. A full cross product would take weeks on one machine.
//!
//! `execute` runs every point without proving, which is enough for gas, instructions, hash calls and
//! witness sizes. The other rounds prove the points that decide something, cheapest first.

use crate::{
    arm::Arm,
    point::{Point, RunMode, Workload},
};

const STATE: u64 = 1_000_000;
/// The height a state of tens of millions of records would need.
const HEIGHT: u32 = 26;

use Arm::*;
use RunMode::{Compressed, Core, Execute};
use Workload::{Append, Insert, Mixed, Update};

const BATCH_ARMS: [Arm; 4] = [
    ImmBatchSha256,
    ImmBatchPoseidon2,
    JmtBatchSha256,
    JmtBatchPoseidon2,
];

fn point(arm: Arm, workload: Workload, k: u32, mode: RunMode) -> Point {
    Point {
        arm,
        workload,
        k,
        sub_batches: 1,
        state: STATE,
        height: if arm.is_jmt() { 0 } else { HEIGHT },
        key_size: if workload == Append { 8 } else { 84 },
        value_size: 24,
        read_proofs: false,
        external_batch_hash: false,
        stop_after: 0,
        mode,
        seed: 1,
    }
}

fn both_ops(arm: Arm, k: u32, mode: RunMode) -> [Point; 2] {
    [point(arm, Insert, k, mode), point(arm, Update, k, mode)]
}

/// Everything, without proofs.
fn execute() -> Vec<Point> {
    let mut points = Vec::new();
    for arm in Arm::ALL {
        for k in [100, 1_000, 10_000] {
            points.extend(both_ops(arm, k, Execute));
        }
        points.push(point(arm, Mixed, 1_000, Execute));
        // The cost of each phase, by difference.
        for stop_after in [1, 2] {
            points.push(Point {
                stop_after,
                ..point(arm, Update, 1_000, Execute)
            });
        }
    }
    for arm in BATCH_ARMS {
        points.extend(both_ops(arm, 100_000, Execute));
        points.push(point(arm, Append, 1_000, Execute));
        points.push(point(arm, Append, 10_000, Execute));
        // Larger records and keys.
        for value_size in [256, 1024] {
            points.push(Point {
                value_size,
                ..point(arm, Insert, 100, Execute)
            });
        }
        points.push(Point {
            key_size: 240,
            ..point(arm, Insert, 100, Execute)
        });
    }
    // Ten blocks in one proof.
    for arm in [ImmBatchSha256, ImmBatchPoseidon2] {
        for op in [Insert, Update] {
            points.push(Point {
                sub_batches: 10,
                ..point(arm, op, 1_000, Execute)
            });
        }
    }
    // A separate read proof per key, on the stock Jellyfish arms.
    for arm in [JmtStockSha2, JmtStockSha256, JmtStockPoseidon2Node] {
        for op in [Insert, Update] {
            for k in [100, 1_000] {
                points.push(Point {
                    read_proofs: true,
                    ..point(arm, op, k, Execute)
                });
            }
        }
    }
    // An external batch commitment on the arms whose own hash has no external verifier.
    for arm in [
        ImmChainedPoseidon2,
        ImmBatchPoseidon2,
        JmtStockPoseidon2Node,
        JmtBatchPoseidon2,
    ] {
        for op in [Insert, Update] {
            points.push(Point {
                external_batch_hash: true,
                ..point(arm, op, 1_000, Execute)
            });
        }
    }
    // The indexed map's cost by height, and by how full the tree is. Height 16 is what fits today's
    // constant, and the small state at greater heights separates the height from the fill.
    for arm in [
        ImmChainedSha256,
        ImmChainedPoseidon2,
        ImmBatchSha256,
        ImmBatchPoseidon2,
    ] {
        for op in [Insert, Update] {
            points.push(Point {
                height: 16,
                state: 16_384,
                ..point(arm, op, 1_000, Execute)
            });
            for height in [21, 28] {
                points.push(Point {
                    height,
                    ..point(arm, op, 1_000, Execute)
                });
            }
            for height in [21, 26] {
                points.push(Point {
                    height,
                    state: 16_384,
                    ..point(arm, op, 1_000, Execute)
                });
            }
        }
    }
    points
}

/// The Jellyfish tree's cost at a deeper state. Run when nothing is proving: the state takes memory.
fn deep() -> Vec<Point> {
    let mut points = Vec::new();
    for arm in [
        JmtStockSha2,
        JmtStockSha256,
        JmtStockPoseidon2Node,
        JmtBatchSha256,
        JmtBatchPoseidon2,
    ] {
        for op in [Insert, Update] {
            points.push(Point {
                state: 10_000_000,
                ..point(arm, op, 1_000, Execute)
            });
        }
    }
    points
}

/// Real proofs at the smallest batch, cheapest first, for every arm but the two slowest.
fn r1() -> Vec<Point> {
    // Today's flow: one insert per proof at today's height.
    let mut points = vec![Point {
        state: 1_000,
        height: 16,
        ..point(ImmChainedSha2, Insert, 1, Core)
    }];
    for arm in [
        JmtBatchPoseidon2,
        ImmBatchPoseidon2,
        ImmChainedPoseidon2,
        JmtStockPoseidon2Node,
        JmtStockPoseidon2Sponge,
        ImmBatchSha256,
        JmtBatchSha256,
        ImmChainedSha256,
        JmtStockSha256,
    ] {
        points.extend(both_ops(arm, 100, Core));
    }
    points
}

/// The arms that hash through the `sha2` crate, at the smallest batch.
fn r1s() -> Vec<Point> {
    let mut points = both_ops(ImmChainedSha2, 100, Core).to_vec();
    points.extend(both_ops(JmtStockSha2, 100, Core));
    points
}

fn r2a() -> Vec<Point> {
    [
        ImmBatchPoseidon2,
        JmtBatchPoseidon2,
        ImmChainedPoseidon2,
        ImmBatchSha256,
        JmtBatchSha256,
    ]
    .into_iter()
    .flat_map(|arm| both_ops(arm, 1_000, Core))
    .collect()
}

fn r2b() -> Vec<Point> {
    let mut points = both_ops(JmtStockPoseidon2Node, 1_000, Core).to_vec();
    points.extend(both_ops(ImmChainedSha256, 1_000, Core));
    points.extend(both_ops(JmtStockSha256, 1_000, Core));
    points.push(point(JmtStockSha2, Update, 1_000, Core));
    points
}

/// The largest batches on the cheapest arms.
fn r3() -> Vec<Point> {
    let mut points = both_ops(ImmBatchPoseidon2, 10_000, Core).to_vec();
    points.extend(both_ops(JmtBatchPoseidon2, 10_000, Core));
    points.push(point(ImmChainedPoseidon2, Update, 10_000, Core));
    points
}

/// The largest batches on the arms that take hours each.
fn r3s() -> Vec<Point> {
    let mut points = vec![point(ImmChainedPoseidon2, Insert, 10_000, Core)];
    points.extend(both_ops(ImmBatchSha256, 10_000, Core));
    points.extend(both_ops(JmtBatchSha256, 10_000, Core));
    points
}

fn r4() -> Vec<Point> {
    let mut points: Vec<Point> = BATCH_ARMS
        .into_iter()
        .map(|arm| point(arm, Mixed, 1_000, Core))
        .collect();
    for arm in [ImmBatchPoseidon2, ImmBatchSha256] {
        points.push(point(arm, Append, 1_000, Core));
        points.push(point(arm, Append, 10_000, Core));
    }
    points
}

/// The Poseidon2 batch arms with a batch commitment anyone can recompute with SHA-256.
fn r5() -> Vec<Point> {
    [ImmBatchPoseidon2, JmtBatchPoseidon2]
        .into_iter()
        .map(|arm| Point {
            external_batch_hash: true,
            ..point(arm, Insert, 1_000, Core)
        })
        .collect()
}

/// Compressed proofs at a range of shard counts: what compression adds depends on the shards.
fn compressed() -> Vec<Point> {
    vec![
        point(JmtBatchPoseidon2, Insert, 1_000, Compressed),
        point(ImmBatchSha256, Insert, 1_000, Compressed),
        point(JmtStockPoseidon2Node, Insert, 1_000, Compressed),
        point(ImmChainedPoseidon2, Update, 10_000, Compressed),
    ]
    .into_iter()
    .chain(BATCH_ARMS.into_iter().map(|arm| Point {
        // What a candidate design costs end to end: a mixed block, committed with SHA-256, compressed.
        external_batch_hash: arm == ImmBatchPoseidon2 || arm == JmtBatchPoseidon2,
        ..point(arm, Mixed, 1_000, Compressed)
    }))
    .collect()
}

pub fn round(name: &str) -> Option<Vec<Point>> {
    Some(match name {
        "execute" => execute(),
        "deep" => deep(),
        "r1" => r1(),
        "r1s" => r1s(),
        "r2a" => r2a(),
        "r2b" => r2b(),
        "r3" => r3(),
        "r3s" => r3s(),
        "r4" => r4(),
        "r5" => r5(),
        "compressed" => compressed(),
        _ => return None,
    })
}
