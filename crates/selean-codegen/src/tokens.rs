//! Design token extraction from a [`Document`].
//!
//! Walks all nodes across all pages to collect unique colors and font families,
//! producing a [`DesignTokens`] summary useful for theming and style audits.

use std::collections::HashMap;

use selean_engine::persistence::Document;
use selean_engine::scene::{Color, SceneGraph, SceneNode, SceneNodeKind};

use crate::tailwind::color_to_hex;

/// A collected color token with usage frequency.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TokenColor {
    /// Hex representation of the color.
    pub hex: String,
    /// Number of times this color appears in the document.
    pub count: usize,
}

/// A collected font token with the set of weights used.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TokenFont {
    /// Font family name.
    pub family: String,
    /// Sorted list of unique font weights used.
    pub weights: Vec<u16>,
}

/// Aggregated design tokens from a document.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DesignTokens {
    /// Unique colors sorted by frequency (most used first).
    pub colors: Vec<TokenColor>,
    /// Unique font families sorted alphabetically.
    pub fonts: Vec<TokenFont>,
}

/// Extracts design tokens from all pages of a [`Document`].
#[must_use]
pub fn extract_tokens(document: &Document) -> DesignTokens {
    let mut color_counts: HashMap<String, usize> = HashMap::new();
    let mut font_weights: HashMap<String, Vec<u16>> = HashMap::new();

    for page in document.pages() {
        collect_from_scene(&page.scene, &mut color_counts, &mut font_weights);
    }

    let colors = build_color_tokens(color_counts);
    let fonts = build_font_tokens(font_weights);

    DesignTokens { colors, fonts }
}

/// Collects color and font data from a single scene graph.
fn collect_from_scene(
    scene: &SceneGraph,
    color_counts: &mut HashMap<String, usize>,
    font_weights: &mut HashMap<String, Vec<u16>>,
) {
    for node in scene.nodes().values() {
        collect_node_colors(node, color_counts);
        collect_node_fonts(node, font_weights);
    }
}

/// Collects all colors from a node's properties.
fn collect_node_colors(node: &SceneNode, counts: &mut HashMap<String, usize>) {
    if let Some(ref fill) = node.fill {
        add_color(counts, fill);
    }
    if let Some(ref stroke) = node.stroke {
        add_color(counts, stroke);
    }
    if let SceneNodeKind::Text {
        text_color: Some(ref tc),
        ..
    } = node.kind
    {
        add_color(counts, tc);
    }
}

/// Collects font family and weight from a text node.
fn collect_node_fonts(node: &SceneNode, font_weights: &mut HashMap<String, Vec<u16>>) {
    if let SceneNodeKind::Text {
        font_family,
        font_weight,
        ..
    } = &node.kind
    {
        let entry = font_weights.entry(font_family.clone()).or_default();
        if !entry.contains(font_weight) {
            entry.push(*font_weight);
        }
    }
}

/// Increments the count for a color hex value.
fn add_color(counts: &mut HashMap<String, usize>, color: &Color) {
    let hex = color_to_hex(color);
    *counts.entry(hex).or_insert(0) += 1;
}

/// Builds sorted color tokens from a frequency map.
fn build_color_tokens(counts: HashMap<String, usize>) -> Vec<TokenColor> {
    let mut tokens: Vec<TokenColor> = counts
        .into_iter()
        .map(|(hex, count)| TokenColor { hex, count })
        .collect();
    tokens.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.hex.cmp(&b.hex)));
    tokens
}

/// Builds sorted font tokens from a family-to-weights map.
fn build_font_tokens(font_weights: HashMap<String, Vec<u16>>) -> Vec<TokenFont> {
    let mut tokens: Vec<TokenFont> = font_weights
        .into_iter()
        .map(|(family, mut weights)| {
            weights.sort_unstable();
            weights.dedup();
            TokenFont { family, weights }
        })
        .collect();
    tokens.sort_by(|a, b| a.family.cmp(&b.family));
    tokens
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use selean_common::types::NodeId;
    use selean_engine::persistence::Page;
    use selean_engine::scene::{BoundingBox, FontStyle, TextAlign};

    use super::*;

    fn make_text_node(family: &str, weight: u16, text_color: Option<Color>) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            "T".to_string(),
            SceneNodeKind::Text {
                content: "text".to_string(),
                font_size: 16.0,
                font_family: family.to_string(),
                font_weight: weight,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 30.0),
        )
    }

    fn make_frame_with_fill(color: Color) -> SceneNode {
        let mut node = SceneNode::new(
            NodeId::new(),
            "F".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.fill = Some(color);
        node
    }

    #[test]
    fn empty_document_yields_empty_tokens() {
        let doc = Document::new();
        let tokens = extract_tokens(&doc);
        assert!(tokens.colors.is_empty());
        assert!(tokens.fonts.is_empty());
    }

    #[test]
    fn color_aggregation() {
        let mut page = Page::new("P1", 800.0, 600.0);
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        page.scene.add_root(make_frame_with_fill(red));
        page.scene.add_root(make_frame_with_fill(red));
        page.scene
            .add_root(make_frame_with_fill(Color::new(0.0, 0.0, 1.0, 1.0)));

        let doc = Document::from_pages(vec![page]);
        let tokens = extract_tokens(&doc);

        assert_eq!(tokens.colors.len(), 2);
        // Red appears twice, should be first.
        assert_eq!(tokens.colors[0].hex, "#ff0000");
        assert_eq!(tokens.colors[0].count, 2);
        assert_eq!(tokens.colors[1].hex, "#0000ff");
        assert_eq!(tokens.colors[1].count, 1);
    }

    #[test]
    fn font_collection() {
        let mut page = Page::new("P1", 800.0, 600.0);
        page.scene.add_root(make_text_node("Inter", 400, None));
        page.scene.add_root(make_text_node("Inter", 700, None));
        page.scene.add_root(make_text_node("Roboto", 400, None));

        let doc = Document::from_pages(vec![page]);
        let tokens = extract_tokens(&doc);

        assert_eq!(tokens.fonts.len(), 2);
        // Alphabetically: Inter, Roboto
        assert_eq!(tokens.fonts[0].family, "Inter");
        assert_eq!(tokens.fonts[0].weights, vec![400, 700]);
        assert_eq!(tokens.fonts[1].family, "Roboto");
        assert_eq!(tokens.fonts[1].weights, vec![400]);
    }

    #[test]
    fn font_weight_dedup() {
        let mut page = Page::new("P1", 800.0, 600.0);
        page.scene.add_root(make_text_node("Inter", 400, None));
        page.scene.add_root(make_text_node("Inter", 400, None));

        let doc = Document::from_pages(vec![page]);
        let tokens = extract_tokens(&doc);

        assert_eq!(tokens.fonts.len(), 1);
        assert_eq!(tokens.fonts[0].weights, vec![400]);
    }

    #[test]
    fn text_color_collected() {
        let mut page = Page::new("P1", 800.0, 600.0);
        let tc = Color::new(0.0, 1.0, 0.0, 1.0);
        page.scene.add_root(make_text_node("Inter", 400, Some(tc)));

        let doc = Document::from_pages(vec![page]);
        let tokens = extract_tokens(&doc);

        assert!(tokens.colors.iter().any(|c| c.hex == "#00ff00"));
    }

    #[test]
    fn multi_page_aggregation() {
        let mut p1 = Page::new("P1", 800.0, 600.0);
        let mut p2 = Page::new("P2", 800.0, 600.0);
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        p1.scene.add_root(make_frame_with_fill(red));
        p2.scene.add_root(make_frame_with_fill(red));

        let doc = Document::from_pages(vec![p1, p2]);
        let tokens = extract_tokens(&doc);

        assert_eq!(tokens.colors.len(), 1);
        assert_eq!(tokens.colors[0].count, 2);
    }
}
