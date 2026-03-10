//! Design-to-code generation engine for Selean.
//!
//! Converts Selean scene graphs and documents into React + Tailwind CSS
//! source code. Supports three levels of output:
//!
//! - Single component: [`generate_component_code`] for a scene graph
//! - Page component: [`generate_page_code`] for a [`Page`]
//! - Full project: [`generate_project`] for a [`Document`]
//!
//! Also provides design token extraction via [`extract_tokens`].

pub mod claude_md;
pub mod jsx;
pub mod project;
pub mod tailwind;
pub mod tokens;

use selean_engine::persistence::{Document, Page};
use selean_engine::scene::SceneGraph;

pub use claude_md::generate_claude_md;
pub use project::{ProjectFile, ProjectOutput};
pub use tokens::{DesignTokens, TokenColor, TokenFont};

/// Generates a React component for a single [`Page`].
///
/// The component name is derived from the page name (converted to `PascalCase`).
#[must_use]
pub fn generate_page_code(page: &Page) -> String {
    jsx::render_scene(&page.scene, &page.name, page.width, page.height)
}

/// Generates a React component for a [`SceneGraph`] with explicit dimensions.
///
/// This is the lower-level entry point when you have a scene graph but no
/// [`Page`] wrapper.
#[must_use]
pub fn generate_component_code(
    scene: &SceneGraph,
    page_name: &str,
    width: f32,
    height: f32,
) -> String {
    jsx::render_scene(scene, page_name, width, height)
}

/// Generates a complete Vite + React + Tailwind project from a [`Document`].
///
/// Returns a [`ProjectOutput`] containing all files needed to run the project.
#[must_use]
pub fn generate_project(document: &Document) -> ProjectOutput {
    project::generate_project(document)
}

/// Extracts design tokens (colors, fonts) from a [`Document`].
///
/// Walks all nodes across all pages to collect unique values.
#[must_use]
pub fn extract_tokens(document: &Document) -> DesignTokens {
    tokens::extract_tokens(document)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use selean_engine::persistence::Page;

    use super::*;

    #[test]
    fn generate_page_code_produces_component() {
        let page = Page::new("Landing Page", 1920.0, 1080.0);
        let code = generate_page_code(&page);
        assert!(code.contains("function LandingPage()"));
        assert!(code.contains("w-[1920px]"));
    }

    #[test]
    fn generate_component_code_works() {
        let scene = SceneGraph::new();
        let code = generate_component_code(&scene, "Test", 800.0, 600.0);
        assert!(code.contains("function Test()"));
    }

    #[test]
    fn generate_project_file_count() {
        let doc = Document::new();
        let output = generate_project(&doc);
        // 7 shared + 1 page component = 8
        assert_eq!(output.files.len(), 8);
    }

    #[test]
    fn extract_tokens_on_empty_doc() {
        let doc = Document::new();
        let tokens = extract_tokens(&doc);
        assert!(tokens.colors.is_empty());
        assert!(tokens.fonts.is_empty());
    }
}
