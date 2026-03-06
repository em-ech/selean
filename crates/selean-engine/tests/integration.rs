//! Integration tests for cross-module workflows.
//!
//! These tests verify that the engine's major subsystems (scene graph,
//! commands, undo/redo, persistence) work correctly together.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use selean_common::types::{NodeId, PageId};
use selean_engine::command::{CommandHistory, SetBoundsCommand, SetFillCommand, SetNameCommand};
use selean_engine::persistence::{Document, load_document, save_document};
use selean_engine::scene::{BoundingBox, Color, FontStyle, SceneNode, SceneNodeKind, TextAlign};

/// Creates a simple document with one page and one rectangle node.
fn setup_document() -> (Document, PageId, NodeId) {
    let mut doc = Document::new();
    let page_id = doc.pages()[0].id;
    let nid = NodeId::new();
    let node = SceneNode::new(
        nid,
        "TestRect".to_string(),
        SceneNodeKind::Frame {
            corner_radius: [0.0; 4],
        },
        BoundingBox::new(10.0, 20.0, 100.0, 50.0),
    );
    let page = doc.page_mut(page_id).expect("default page exists");
    page.scene.add_root(node);
    (doc, page_id, nid)
}

#[test]
fn create_node_save_load_roundtrip() {
    let (doc, page_id, node_id) = setup_document();

    // Save.
    let json = save_document(&doc).expect("save should succeed");

    // Load.
    let loaded = load_document(&json).expect("load should succeed");
    let page = loaded.page(page_id).expect("page should exist");
    let node = page.scene.get(node_id).expect("node should exist");

    assert_eq!(node.name, "TestRect");
    assert_eq!(node.bounds.x, 10.0);
    assert_eq!(node.bounds.y, 20.0);
    assert_eq!(node.bounds.width, 100.0);
    assert_eq!(node.bounds.height, 50.0);
}

#[test]
fn command_execute_and_undo_roundtrip() {
    let (mut doc, page_id, node_id) = setup_document();
    let page = doc.page_mut(page_id).expect("page");
    let mut history = CommandHistory::new();

    // Set fill via command.
    let fill_color = Color::new(1.0, 0.0, 0.0, 1.0);
    let cmd = SetFillCommand::new(node_id, Some(fill_color));
    assert!(history.execute(Box::new(cmd), &mut page.scene));

    // Verify fill was applied.
    let node = page.scene.get(node_id).expect("node");
    assert_eq!(node.fill, Some(fill_color));

    // Undo.
    assert!(history.undo(&mut page.scene));
    let node = page.scene.get(node_id).expect("node");
    assert_eq!(node.fill, None);

    // Redo.
    assert!(history.redo(&mut page.scene));
    let node = page.scene.get(node_id).expect("node");
    assert_eq!(node.fill, Some(fill_color));
}

#[test]
fn command_then_persist_preserves_state() {
    let (mut doc, page_id, node_id) = setup_document();

    // Apply commands.
    {
        let page = doc.page_mut(page_id).expect("page");
        let mut history = CommandHistory::new();

        let name_cmd = SetNameCommand::new(node_id, "Renamed".to_string());
        history.execute(Box::new(name_cmd), &mut page.scene);

        let bounds_cmd = SetBoundsCommand::new(node_id, BoundingBox::new(50.0, 60.0, 200.0, 150.0));
        history.execute(Box::new(bounds_cmd), &mut page.scene);
    }

    // Save and reload.
    let json = save_document(&doc).expect("save");
    let loaded = load_document(&json).expect("load");
    let page = loaded.page(page_id).expect("page");
    let node = page.scene.get(node_id).expect("node");

    assert_eq!(node.name, "Renamed");
    assert_eq!(node.bounds.x, 50.0);
    assert_eq!(node.bounds.y, 60.0);
    assert_eq!(node.bounds.width, 200.0);
    assert_eq!(node.bounds.height, 150.0);
}

#[test]
fn multi_page_document_roundtrip() {
    let mut doc = Document::new();
    let page1_id = doc.pages()[0].id;

    // Add a second page.
    let page2_id = PageId::new();
    doc.add_page_with_id(page2_id, "Page 2", 800.0, 600.0);

    // Add nodes to each page.
    let n1_id = NodeId::new();
    {
        let page1 = doc.page_mut(page1_id).expect("page1");
        let n1 = SceneNode::new(
            n1_id,
            "P1Rect".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        page1.scene.add_root(n1);
    }
    let n2_id = NodeId::new();
    {
        let page2 = doc.page_mut(page2_id).expect("page2");
        let n2 = SceneNode::new(
            n2_id,
            "P2Text".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 24.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::default(),
                text_align: TextAlign::default(),
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(10.0, 10.0, 200.0, 50.0),
        );
        page2.scene.add_root(n2);
    }

    // Roundtrip.
    let json = save_document(&doc).expect("save");
    let loaded = load_document(&json).expect("load");

    assert_eq!(loaded.pages().len(), 2);

    let p1 = loaded.page(page1_id).expect("page1");
    assert_eq!(p1.scene.roots().len(), 1);
    let node1 = p1.scene.get(n1_id).expect("node");
    assert_eq!(node1.name, "P1Rect");

    let p2 = loaded.page(page2_id).expect("page2");
    assert_eq!(p2.scene.roots().len(), 1);
    let node2 = p2.scene.get(n2_id).expect("node");
    assert_eq!(node2.name, "P2Text");
    match &node2.kind {
        SceneNodeKind::Text {
            content, font_size, ..
        } => {
            assert_eq!(content, "Hello");
            assert!((font_size - 24.0).abs() < f32::EPSILON);
        }
        other => panic!("expected Text, got: {other:?}"),
    }
}

#[test]
fn undo_across_multiple_commands_restores_original_state() {
    let (mut doc, page_id, node_id) = setup_document();
    let page = doc.page_mut(page_id).expect("page");
    let scene = &mut page.scene;
    let mut history = CommandHistory::new();

    // Save original state.
    let original_bounds = scene.get(node_id).expect("node").bounds;

    // Apply 3 commands.
    let cmd1 = SetNameCommand::new(node_id, "Step1".to_string());
    history.execute(Box::new(cmd1), scene);

    let cmd2 = SetBoundsCommand::new(node_id, BoundingBox::new(0.0, 0.0, 500.0, 500.0));
    history.execute(Box::new(cmd2), scene);

    let cmd3 = SetFillCommand::new(node_id, Some(Color::new(0.0, 1.0, 0.0, 1.0)));
    history.execute(Box::new(cmd3), scene);

    // Undo all 3.
    assert!(history.undo(scene)); // undo fill
    assert!(history.undo(scene)); // undo bounds
    assert!(history.undo(scene)); // undo name

    let node = scene.get(node_id).expect("node");
    assert_eq!(node.name, "TestRect");
    assert_eq!(node.bounds, original_bounds);
    assert_eq!(node.fill, None);
}
