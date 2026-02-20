//! Criterion benchmarks for the Selean engine.
//!
//! Four benchmark groups across three scale tiers (1K, 10K, 50K):
//! 1. Scene mutation — bulk insert, individual insert/remove, property mutations
//! 2. Spatial queries — viewport, point, region queries
//! 3. Render preparation — `visible_nodes_sorted` with varying viewport sizes
//! 4. Full frame simulation — combined mutation + query + render prep

#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    missing_docs
)]

mod bench_utils;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rand::prelude::*;
use rand::rngs::StdRng;

use selean_common::types::NodeId;
use selean_engine::image::{decode_image, decode_image_resized};
use selean_engine::renderer::{RectBatch, TexturedQuadBatch};
use selean_engine::scene::{BlendMode, BoundingBox, Color, SceneNode, SceneNodeKind, Transform2D};
use selean_engine::text::{
    AtlasRegion, FontData, GlyphCache, GlyphCacheKey, SdfParams, ShapedRun, TextBatch,
    generate_glyph_sdf, layout_text, shape_text,
};
use selean_engine::vector::{
    cache::quantize_dimension, parser::parse_path_data, rasterizer::rasterize_path,
};

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

// ---------------------------------------------------------------------------
// Group 5: Text shaping
// ---------------------------------------------------------------------------

fn bench_text_shaping(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_shaping");
    let font = FontData::default_font().expect("default font");

    let inputs: &[(&str, &str)] = &[
        ("short_5", "Hello"),
        (
            "medium_50",
            "The quick brown fox jumps over the lazy dog nearby",
        ),
        (
            "long_200",
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut.",
        ),
    ];

    for &(label, text) in inputs {
        group.bench_with_input(BenchmarkId::new("shape", label), &text, |b, &text| {
            b.iter(|| {
                black_box(shape_text(&font, text));
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 6: SDF generation
// ---------------------------------------------------------------------------

fn bench_sdf_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sdf_generation");
    let font = FontData::default_font().expect("default font");
    let face = font.face().expect("font face");

    let glyphs: &[(&str, char)] = &[
        ("letter_A", 'A'),
        ("letter_g", 'g'),
        ("letter_W", 'W'),
        ("digit_0", '0'),
        ("ampersand", '&'),
    ];

    let params = SdfParams::default();

    for &(label, ch) in glyphs {
        let glyph_id = face.glyph_index(ch).expect("glyph index");
        group.bench_with_input(BenchmarkId::new("generate", label), &glyph_id, |b, &gid| {
            b.iter(|| {
                black_box(generate_glyph_sdf(&face, gid, &params));
            });
        });
    }

    // Bench different SDF sizes.
    let glyph_a = face.glyph_index('A').expect("glyph index");
    for &size in &[32u32, 48, 64, 96] {
        let p = SdfParams {
            render_size: size,
            spread: 6,
        };
        group.bench_with_input(BenchmarkId::new("size", size), &size, |b, _| {
            b.iter(|| {
                black_box(generate_glyph_sdf(&face, glyph_a, &p));
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 7: Text layout
// ---------------------------------------------------------------------------

fn bench_text_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_layout");
    let font = FontData::default_font().expect("default font");

    // Pre-shape some text and populate a cache.
    let short = "Hello";
    let medium = "The quick brown fox jumps over the lazy dog nearby";
    let long: String = "Pack my box with five dozen liquor jugs. ".repeat(5);

    let shaped_short = shape_text(&font, short).expect("shape short");
    let shaped_medium = shape_text(&font, medium).expect("shape medium");
    let shaped_long = shape_text(&font, &long).expect("shape long");

    // Build a cache containing all glyphs we'll need.
    let cache = build_bench_cache(&font, &[&shaped_short, &shaped_medium, &shaped_long]);

    let ascender = font.ascender();

    let cases: &[(&str, &ShapedRun)] = &[
        ("short_5", &shaped_short),
        ("medium_50", &shaped_medium),
        ("long_200", &shaped_long),
    ];

    for &(label, run) in cases {
        group.bench_with_input(BenchmarkId::new("layout", label), &label, |b, _| {
            b.iter(|| {
                black_box(layout_text(run, &cache, ascender, 100.0, 200.0, 16.0, 48));
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 8: Combined text processing (shape + SDF + layout)
// ---------------------------------------------------------------------------

fn bench_text_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_full_pipeline");
    group.sample_size(50);

    let font = FontData::default_font().expect("default font");
    let params = SdfParams::default();

    let texts: &[(&str, &str)] = &[
        ("short", "Hello"),
        ("sentence", "The quick brown fox jumps over the lazy dog"),
    ];

    for &(label, text) in texts {
        group.bench_with_input(
            BenchmarkId::new("shape_sdf_layout", label),
            &text,
            |b, &text| {
                b.iter(|| {
                    // Shape.
                    let shaped = shape_text(&font, text).expect("shape");

                    // Generate SDFs (cold cache — worst case).
                    let face = font.face().expect("face");
                    let mut cache = GlyphCache::new();
                    for sg in &shaped.glyphs {
                        #[allow(clippy::cast_possible_truncation)]
                        let key = GlyphCacheKey {
                            glyph_id: sg.glyph_id,
                            sdf_size: params.render_size as u16,
                        };
                        if !cache.contains(&key) {
                            let glyph_id = ttf_parser::GlyphId(sg.glyph_id);
                            if let Some(sdf) = generate_glyph_sdf(&face, glyph_id, &params) {
                                cache.insert(
                                    key,
                                    selean_engine::text::CachedGlyph {
                                        atlas_region: selean_engine::text::AtlasRegion {
                                            x: 0,
                                            y: 0,
                                            width: sdf.width,
                                            height: sdf.height,
                                        },
                                        bearing_x: sdf.bearing_x,
                                        bearing_y: sdf.bearing_y,
                                        glyph_width_funits: sdf.glyph_width_funits,
                                        glyph_height_funits: sdf.glyph_height_funits,
                                    },
                                );
                            }
                        }
                    }

                    // Layout.
                    let layout =
                        layout_text(&shaped, &cache, font.ascender(), 100.0, 200.0, 16.0, 48);

                    black_box(layout);
                });
            },
        );
    }

    // Warm-cache benchmark: SDF generation only happens once, layout is repeated.
    group.bench_function("layout_warm_cache_sentence", |b| {
        let text = "The quick brown fox jumps over the lazy dog";
        let shaped = shape_text(&font, text).expect("shape");
        let cache = build_bench_cache(&font, &[&shaped]);

        b.iter(|| {
            black_box(layout_text(
                &shaped,
                &cache,
                font.ascender(),
                100.0,
                200.0,
                16.0,
                48,
            ));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 9: CPU batching
// ---------------------------------------------------------------------------

fn bench_cpu_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_batching");

    // Pre-build scenes with visible rect and text nodes.
    let rect_nodes: Vec<SceneNode> = (0..1000)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let x = (i as f32) * 10.0;
            let mut node = SceneNode::new(
                NodeId::new(),
                format!("Rect-{i}"),
                SceneNodeKind::Frame {
                    corner_radius: [4.0; 4],
                },
                BoundingBox::new(x, 0.0, 80.0, 40.0),
            );
            node.fill = Some(Color::new(0.2, 0.4, 0.8, 1.0));
            node.stroke = Some(Color::new(0.0, 0.0, 0.0, 1.0));
            node.stroke_width = 1.0;
            node
        })
        .collect();

    for &count in &[100_usize, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("rect_batch_push", count),
            &count,
            |b, &count| {
                let nodes = &rect_nodes[..count];
                b.iter(|| {
                    let mut batch = RectBatch::new();
                    for node in nodes {
                        batch.push_node(node);
                    }
                    black_box(batch.len());
                });
            },
        );
    }

    // Bench TextBatch with pre-built glyph instances.
    let glyph_instance: selean_engine::text::GlyphInstance = bytemuck::Zeroable::zeroed();
    for &count in &[100_usize, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("text_batch_push", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut batch = TextBatch::new();
                    for _ in 0..count {
                        batch.push(glyph_instance);
                    }
                    black_box(batch.len());
                });
            },
        );
    }

    // Bench as_bytes (bytemuck cast) for various batch sizes.
    for &count in &[100_usize, 1000] {
        group.bench_with_input(
            BenchmarkId::new("rect_batch_as_bytes", count),
            &count,
            |b, &count| {
                let mut batch = RectBatch::new();
                for node in &rect_nodes[..count] {
                    batch.push_node(node);
                }
                b.iter(|| {
                    black_box(batch.as_bytes().len());
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 10: Image decode
// ---------------------------------------------------------------------------

fn bench_image_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_decode");

    // Create test PNGs of various sizes.
    let sizes: &[(u32, u32, &str)] = &[
        (64, 64, "64x64"),
        (256, 256, "256x256"),
        (1024, 1024, "1024x1024"),
    ];

    for &(w, h, label) in sizes {
        let png = make_test_png(w, h);
        group.bench_with_input(BenchmarkId::new("decode", label), &png, |b, png| {
            b.iter(|| {
                black_box(decode_image(png).expect("decode"));
            });
        });
    }

    // Decode + resize.
    let large_png = make_test_png(512, 512);
    group.bench_function("decode_resize_512_to_128", |b| {
        b.iter(|| {
            black_box(decode_image_resized(&large_png, 128).expect("decode_resized"));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 11: Vector parse + rasterize
// ---------------------------------------------------------------------------

fn bench_vector_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_pipeline");

    let paths: &[(&str, &str)] = &[
        ("triangle", "M 0 0 L 50 0 L 25 50 Z"),
        ("rect", "M 0 0 L 100 0 L 100 100 L 0 100 Z"),
        ("cubic", "M 10 80 C 40 10 65 10 95 80 S 150 150 10 80"),
        (
            "complex",
            "M 0 0 C 20 40 60 40 80 0 L 80 60 Q 40 100 0 60 Z",
        ),
    ];

    // Parse benchmarks.
    for &(label, path_data) in paths {
        group.bench_with_input(
            BenchmarkId::new("parse", label),
            &path_data,
            |b, &path_data| {
                b.iter(|| {
                    black_box(parse_path_data(path_data).expect("parse"));
                });
            },
        );
    }

    // Rasterize benchmarks at different sizes.
    for &(label, path_data) in paths {
        let path = parse_path_data(path_data).expect("parse");
        for &size in &[64u32, 128, 256] {
            group.bench_with_input(
                BenchmarkId::new(format!("rasterize_{label}"), size),
                &size,
                |b, &size| {
                    b.iter(|| {
                        black_box(
                            rasterize_path(&path, size, size, Some(Color::BLACK), None, 0.0)
                                .expect("rasterize"),
                        );
                    });
                },
            );
        }
    }

    // Quantize dimension benchmark (very fast, verify negligible cost).
    group.bench_function("quantize_dimension", |b| {
        let dims: Vec<f32> = (1..=500).map(|i| i as f32).collect();
        b.iter(|| {
            for &d in &dims {
                black_box(quantize_dimension(d));
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 12: Image/vector batch CPU operations
// ---------------------------------------------------------------------------

fn bench_textured_quad_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("textured_quad_batching");

    // Build image-like nodes for batching.
    let image_nodes: Vec<SceneNode> = (0..1000)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let x = (i as f32) * 10.0;
            let mut node = SceneNode::new(
                NodeId::new(),
                format!("Img-{i}"),
                SceneNodeKind::Image {
                    asset_ref: format!("asset_{i}.png"),
                },
                BoundingBox::new(x, 0.0, 80.0, 60.0),
            );
            node.fill = Some(Color::new(1.0, 1.0, 1.0, 1.0));
            node.opacity = 1.0;
            node
        })
        .collect();

    let region = AtlasRegion {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };

    for &count in &[100_usize, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("push_pending", count),
            &count,
            |b, &count| {
                let nodes = &image_nodes[..count];
                b.iter(|| {
                    let mut batch = TexturedQuadBatch::new();
                    for node in nodes {
                        batch.push_pending(node, region, node.fill);
                    }
                    black_box(batch.pending_count());
                });
            },
        );
    }

    // Finalize UVs benchmark.
    for &count in &[100_usize, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("finalize_uvs", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let mut batch = TexturedQuadBatch::new();
                        for node in &image_nodes[..count] {
                            batch.push_pending(node, region, node.fill);
                        }
                        batch
                    },
                    |mut batch| {
                        batch.finalize_uvs(2048, 2048);
                        black_box(batch.len());
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 13: Transform mutations + world transform recomputation
// ---------------------------------------------------------------------------

fn bench_transforms(c: &mut Criterion) {
    let mut group = c.benchmark_group("transforms");

    for &count in TIERS {
        let scene = generate_scene(&SceneConfig::with_count(count));
        let all_ids: Vec<NodeId> = scene
            .roots()
            .iter()
            .flat_map(|&r| {
                let mut ids = vec![r];
                ids.extend(scene.descendants(r));
                ids
            })
            .collect();

        // set_transform on 100 nodes (marks descendants dirty).
        group.bench_with_input(
            BenchmarkId::new("set_transform_100", count),
            &count,
            |b, _| {
                let ids: Vec<NodeId> = all_ids.iter().copied().take(100).collect();
                let mut rng = StdRng::seed_from_u64(77);
                let transforms: Vec<Transform2D> = (0..100)
                    .map(|_| {
                        let angle = rng
                            .gen_range(-std::f32::consts::FRAC_PI_4..std::f32::consts::FRAC_PI_4);
                        Transform2D::rotation(angle)
                    })
                    .collect();
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        for (id, t) in ids.iter().zip(transforms.iter()) {
                            scene.set_transform(*id, *t);
                        }
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // set_rotation on 100 nodes (convenience wrapper).
        group.bench_with_input(
            BenchmarkId::new("set_rotation_100", count),
            &count,
            |b, _| {
                let ids: Vec<NodeId> = all_ids.iter().copied().take(100).collect();
                let mut rng = StdRng::seed_from_u64(88);
                let angles: Vec<f32> = (0..100)
                    .map(|_| rng.gen_range(-std::f32::consts::PI..std::f32::consts::PI))
                    .collect();
                b.iter_batched(
                    || scene.clone(),
                    |mut scene| {
                        for (id, &a) in ids.iter().zip(angles.iter()) {
                            scene.set_rotation(*id, a);
                        }
                        black_box(scene);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // World transform recomputation after dirtying 100 nodes.
        group.bench_with_input(
            BenchmarkId::new("recompute_world_transforms", count),
            &count,
            |b, _| {
                let ids: Vec<NodeId> = all_ids.iter().copied().take(100).collect();
                b.iter_batched(
                    || {
                        let mut s = scene.clone();
                        for id in &ids {
                            s.set_transform(*id, Transform2D::rotation(0.1));
                        }
                        s
                    },
                    |mut scene| {
                        scene.recompute_world_transforms();
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
// Group 14: Hit testing (with rotated nodes)
// ---------------------------------------------------------------------------

fn bench_hit_testing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_testing");

    for &count in TIERS {
        // Scene with 20% rotated nodes.
        let config = SceneConfig {
            node_count: count,
            transform_fraction: 0.2,
            ..SceneConfig::default()
        };
        let mut scene = generate_scene(&config);

        // hit_test at center of canvas.
        group.bench_with_input(BenchmarkId::new("center", count), &count, |b, _| {
            b.iter(|| {
                black_box(scene.hit_test(5000.0, 5000.0));
            });
        });

        // hit_test at corner (fewer hits expected).
        group.bench_with_input(BenchmarkId::new("corner", count), &count, |b, _| {
            b.iter(|| {
                black_box(scene.hit_test(100.0, 100.0));
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 15: Blend mode batch splitting
// ---------------------------------------------------------------------------

fn bench_blend_mode_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("blend_mode_batching");

    // Build rect nodes with varying blend modes.
    for &add_fraction in &[0.0_f32, 0.05, 0.20] {
        let label = format!("add_{:.0}pct", add_fraction * 100.0);
        let nodes: Vec<SceneNode> = (0..1000)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let x = (i as f32) * 10.0;
                let mut node = SceneNode::new(
                    NodeId::new(),
                    format!("Rect-{i}"),
                    SceneNodeKind::Frame {
                        corner_radius: [4.0; 4],
                    },
                    BoundingBox::new(x, 0.0, 80.0, 40.0),
                );
                node.fill = Some(Color::new(0.2, 0.4, 0.8, 1.0));
                node.opacity = 1.0;
                #[allow(clippy::cast_precision_loss)]
                if (i as f32) < 1000.0 * add_fraction {
                    node.blend_mode = BlendMode::Add;
                }
                node
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("rect_push_1000", &label),
            &label,
            |b, _| {
                b.iter(|| {
                    let mut batch = RectBatch::new();
                    for node in &nodes {
                        batch.push_node(node);
                    }
                    batch.finalize_blend();
                    black_box(batch.len());
                });
            },
        );
    }

    group.finish();
}

/// Creates a minimal valid RGBA PNG for benchmarking.
fn make_test_png(width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        let data: Vec<u8> = (0..(width * height))
            .flat_map(|_| [255u8, 0, 0, 255])
            .collect();
        image::ImageEncoder::write_image(
            encoder,
            &data,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode test png");
    }
    buf
}

/// Helper: builds a `GlyphCache` with SDF metadata for all unique glyphs in the given runs.
///
/// Uses fake atlas regions (all at origin) — sufficient for layout benchmarking.
fn build_bench_cache(font: &FontData, runs: &[&ShapedRun]) -> GlyphCache {
    let face = font.face().expect("font face");
    let params = SdfParams::default();
    let mut cache = GlyphCache::new();

    #[allow(clippy::cast_possible_truncation)]
    let sdf_size = params.render_size as u16;

    for run in runs {
        for sg in &run.glyphs {
            let key = GlyphCacheKey {
                glyph_id: sg.glyph_id,
                sdf_size,
            };
            if cache.contains(&key) {
                continue;
            }

            let glyph_id = ttf_parser::GlyphId(sg.glyph_id);
            if let Some(sdf) = generate_glyph_sdf(&face, glyph_id, &params) {
                cache.insert(
                    key,
                    selean_engine::text::CachedGlyph {
                        atlas_region: selean_engine::text::AtlasRegion {
                            x: 0,
                            y: 0,
                            width: sdf.width,
                            height: sdf.height,
                        },
                        bearing_x: sdf.bearing_x,
                        bearing_y: sdf.bearing_y,
                        glyph_width_funits: sdf.glyph_width_funits,
                        glyph_height_funits: sdf.glyph_height_funits,
                    },
                );
            }
        }
    }

    cache
}

criterion_group!(
    benches,
    bench_scene_mutation,
    bench_spatial_queries,
    bench_render_prep,
    bench_full_frame,
    bench_text_shaping,
    bench_sdf_generation,
    bench_text_layout,
    bench_text_full_pipeline,
    bench_cpu_batching,
    bench_image_decode,
    bench_vector_pipeline,
    bench_textured_quad_batching,
    bench_transforms,
    bench_hit_testing,
    bench_blend_mode_batching,
);
criterion_main!(benches);
