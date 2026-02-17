//! Criterion benchmarks for the Selean engine.
//!
//! Four benchmark groups across three scale tiers (1K, 10K, 50K):
//! 1. Scene mutation — bulk insert, individual insert/remove, property mutations
//! 2. Spatial queries — viewport, point, region queries
//! 3. Render preparation — `visible_nodes_sorted` with varying viewport sizes
//! 4. Full frame simulation — combined mutation + query + render prep

#![allow(clippy::expect_used, clippy::too_many_lines, missing_docs)]

mod bench_utils;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rand::prelude::*;
use rand::rngs::StdRng;

use selean_common::types::NodeId;
use selean_engine::scene::{BoundingBox, Color, SceneNode, SceneNodeKind};

use bench_utils::{SceneConfig, generate_scene};

const TIERS: &[usize] = &[1_000, 10_000, 50_000];

// ---------------------------------------------------------------------------
// Group 1: Scene mutation
// ---------------------------------------------------------------------------

fn bench_scene_mutation(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_mutation");

    for &count in TIERS {
        // Bulk insert: build a scene from scratch.
        group.bench_with_input(
            BenchmarkId::new("bulk_insert", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let _ = black_box(generate_scene(&SceneConfig::with_count(count)));
                });
            },
        );

        // Individual insert: add one node to an existing scene.
        group.bench_with_input(
            BenchmarkId::new("single_insert", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                let roots: Vec<NodeId> = scene.roots().to_vec();
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        let node = SceneNode::new(
                            NodeId::new(),
                            String::from("New-Node"),
                            SceneNodeKind::Frame {
                                corner_radius: [0.0; 4],
                            },
                            BoundingBox::new(100.0, 100.0, 50.0, 50.0),
                        );
                        if roots.is_empty() {
                            scene.add_root(node);
                        } else {
                            scene.add_child(roots[0], node);
                        }
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Remove: remove a root node (triggers subtree removal).
        group.bench_with_input(
            BenchmarkId::new("remove_root", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                let first_root = scene.roots()[0];
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        scene.remove(first_root);
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Property mutation: set_bounds on 100 random nodes.
        group.bench_with_input(
            BenchmarkId::new("set_bounds_100", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                let mut rng = StdRng::seed_from_u64(123);
                // Collect some node IDs to mutate.
                let all_ids: Vec<NodeId> = scene
                    .roots()
                    .iter()
                    .flat_map(|&r| {
                        let mut ids = vec![r];
                        ids.extend(scene.descendants(r));
                        ids
                    })
                    .take(100)
                    .collect();
                let bounds_list: Vec<BoundingBox> = (0..100)
                    .map(|_| {
                        BoundingBox::new(
                            rng.gen_range(0.0_f32..1000.0),
                            rng.gen_range(0.0_f32..1000.0),
                            rng.gen_range(20.0_f32..200.0),
                            rng.gen_range(20.0_f32..200.0),
                        )
                    })
                    .collect();
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        for (id, bounds) in all_ids.iter().zip(bounds_list.iter()) {
                            scene.set_bounds(*id, *bounds);
                        }
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Fill mutation: set_fill on 100 random nodes.
        group.bench_with_input(
            BenchmarkId::new("set_fill_100", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                let all_ids: Vec<NodeId> = scene
                    .roots()
                    .iter()
                    .flat_map(|&r| {
                        let mut ids = vec![r];
                        ids.extend(scene.descendants(r));
                        ids
                    })
                    .take(100)
                    .collect();
                let color = Color::new(1.0, 0.0, 0.0, 1.0);
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        for id in &all_ids {
                            scene.set_fill(*id, Some(color));
                        }
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 2: Spatial queries
// ---------------------------------------------------------------------------

fn bench_spatial_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial_queries");

    for &count in TIERS {
        let scene = generate_scene(&SceneConfig::with_count(count));
        let spatial = scene.spatial();

        // Viewport query: center 1/4 of canvas.
        group.bench_with_input(
            BenchmarkId::new("viewport_quarter", count),
            &count,
            |b, _| {
                b.iter(|| {
                    black_box(spatial.query_viewport(2500.0, 2500.0, 7500.0, 7500.0));
                });
            },
        );

        // Viewport query: center 10% of canvas.
        group.bench_with_input(BenchmarkId::new("viewport_10pct", count), &count, |b, _| {
            b.iter(|| {
                black_box(spatial.query_viewport(4500.0, 4500.0, 5500.0, 5500.0));
            });
        });

        // Viewport query: full canvas.
        group.bench_with_input(BenchmarkId::new("viewport_full", count), &count, |b, _| {
            b.iter(|| {
                black_box(spatial.query_viewport(0.0, 0.0, 10_000.0, 10_000.0));
            });
        });

        // Point query: center of canvas.
        group.bench_with_input(BenchmarkId::new("point_center", count), &count, |b, _| {
            b.iter(|| {
                black_box(spatial.query_point(5000.0, 5000.0));
            });
        });

        // Region query: small 200x200 region.
        group.bench_with_input(BenchmarkId::new("region_small", count), &count, |b, _| {
            b.iter(|| {
                black_box(spatial.query_region(4900.0, 4900.0, 5100.0, 5100.0));
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 3: Render preparation
// ---------------------------------------------------------------------------

fn bench_render_prep(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_preparation");

    for &count in TIERS {
        // Full viewport.
        group.bench_with_input(
            BenchmarkId::new("visible_sorted_full", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        black_box(scene.visible_nodes_sorted(0.0, 0.0, 10_000.0, 10_000.0));
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Quarter viewport (center).
        group.bench_with_input(
            BenchmarkId::new("visible_sorted_quarter", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        black_box(scene.visible_nodes_sorted(2500.0, 2500.0, 7500.0, 7500.0));
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // 10% viewport (center).
        group.bench_with_input(
            BenchmarkId::new("visible_sorted_10pct", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        black_box(scene.visible_nodes_sorted(4500.0, 4500.0, 5500.0, 5500.0));
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 4: Full frame simulation
// ---------------------------------------------------------------------------

fn bench_full_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_frame");
    // Allow longer measurement time for heavy benchmarks.
    group.sample_size(50);

    for &count in TIERS {
        // Simulates one frame: mutate some nodes → query viewport → sort for rendering.
        group.bench_with_input(
            BenchmarkId::new("frame_with_mutations", count),
            &count,
            |b, &count| {
                let base_scene = generate_scene(&SceneConfig::with_count(count));
                let mutation_ids: Vec<NodeId> = base_scene
                    .roots()
                    .iter()
                    .flat_map(|&r| {
                        let mut ids = vec![r];
                        ids.extend(base_scene.descendants(r));
                        ids
                    })
                    .take(50)
                    .collect();

                let mut rng = StdRng::seed_from_u64(99);
                let bounds_list: Vec<BoundingBox> = (0..50)
                    .map(|_| {
                        BoundingBox::new(
                            rng.gen_range(0.0_f32..1000.0),
                            rng.gen_range(0.0_f32..1000.0),
                            rng.gen_range(20.0_f32..200.0),
                            rng.gen_range(20.0_f32..200.0),
                        )
                    })
                    .collect();

                b.iter_batched(
                    || base_scene.clone(),
                    |mut scene| {
                        // Step 1: Mutate 50 nodes (simulating user dragging / property edits).
                        for (id, bounds) in mutation_ids.iter().zip(bounds_list.iter()) {
                            scene.set_bounds(*id, *bounds);
                        }

                        // Step 2: Query visible nodes in a typical 1920x1080 viewport.
                        let visible = scene.visible_nodes_sorted(0.0, 0.0, 1920.0, 1080.0);
                        black_box(visible.len());

                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Read-only frame: no mutations, just viewport query + sort.
        group.bench_with_input(
            BenchmarkId::new("frame_read_only", count),
            &count,
            |b, &count| {
                let scene = generate_scene(&SceneConfig::with_count(count));
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        let visible = scene.visible_nodes_sorted(0.0, 0.0, 1920.0, 1080.0);
                        black_box(visible.len());
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_scene_mutation,
    bench_spatial_queries,
    bench_render_prep,
    bench_full_frame,
);
criterion_main!(benches);
