//! Multi-page document model.
//!
//! A [`Document`] holds a collection of [`Page`] instances and tracks
//! which page is currently active for editing and rendering.

use selean_common::types::PageId;

use super::page::Page;
use crate::scene::SceneGraph;

/// A multi-page document containing one or more pages.
///
/// The document tracks an active page index. All rendering and editing
/// operations target the active page's scene graph.
#[derive(Clone)]
pub struct Document {
    /// Ordered list of pages.
    pages: Vec<Page>,
    /// Index into `pages` for the currently active page.
    active_page_index: usize,
}

impl Document {
    /// Creates a new document with a single default page.
    #[must_use]
    pub fn new() -> Self {
        let page = Page::new("Page 1", 1920.0, 1080.0);
        Self {
            pages: vec![page],
            active_page_index: 0,
        }
    }

    /// Creates a document from a list of pages.
    ///
    /// # Panics
    ///
    /// Panics if `pages` is empty.
    #[must_use]
    pub fn from_pages(pages: Vec<Page>) -> Self {
        assert!(!pages.is_empty(), "Document must have at least one page");
        Self {
            pages,
            active_page_index: 0,
        }
    }

    /// Adds a new empty page and returns its ID.
    pub fn add_page(&mut self, name: impl Into<String>, width: f32, height: f32) -> PageId {
        let page = Page::new(name, width, height);
        let id = page.id;
        self.pages.push(page);
        id
    }

    /// Adds a new empty page with a specific ID and returns `true` if inserted.
    ///
    /// Returns `false` if a page with the given ID already exists.
    pub fn add_page_with_id(
        &mut self,
        id: PageId,
        name: impl Into<String>,
        width: f32,
        height: f32,
    ) -> bool {
        if self.pages.iter().any(|p| p.id == id) {
            return false;
        }
        let page = Page::with_scene(id, name, width, height, SceneGraph::new());
        self.pages.push(page);
        true
    }

    /// Removes a page by ID.
    ///
    /// Returns `false` if the page was not found or if it is the last page
    /// (a document must always have at least one page).
    pub fn remove_page(&mut self, id: PageId) -> bool {
        if self.pages.len() <= 1 {
            return false;
        }
        let Some(index) = self.pages.iter().position(|p| p.id == id) else {
            return false;
        };
        self.pages.remove(index);
        // Adjust active index if needed.
        if self.active_page_index >= self.pages.len() {
            self.active_page_index = self.pages.len() - 1;
        }
        true
    }

    /// Returns a reference to the active page.
    #[must_use]
    pub fn active_page(&self) -> &Page {
        &self.pages[self.active_page_index]
    }

    /// Returns a mutable reference to the active page.
    #[must_use]
    pub fn active_page_mut(&mut self) -> &mut Page {
        &mut self.pages[self.active_page_index]
    }

    /// Returns the index of the active page.
    #[must_use]
    pub fn active_page_index(&self) -> usize {
        self.active_page_index
    }

    /// Switches the active page by ID.
    ///
    /// Returns `false` if the page was not found.
    pub fn set_active_page(&mut self, id: PageId) -> bool {
        if let Some(index) = self.pages.iter().position(|p| p.id == id) {
            self.active_page_index = index;
            true
        } else {
            false
        }
    }

    /// Returns a reference to a page by ID.
    #[must_use]
    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.pages.iter().find(|p| p.id == id)
    }

    /// Returns a mutable reference to a page by ID.
    #[must_use]
    pub fn page_mut(&mut self, id: PageId) -> Option<&mut Page> {
        self.pages.iter_mut().find(|p| p.id == id)
    }

    /// Returns a slice of all pages.
    #[must_use]
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Returns the number of pages.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("page_count", &self.pages.len())
            .field("active_page_index", &self.active_page_index)
            .finish()
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn new_document_has_one_page() {
        let doc = Document::new();
        assert_eq!(doc.page_count(), 1);
        assert_eq!(doc.active_page().name, "Page 1");
        assert_eq!(doc.active_page().width, 1920.0);
        assert_eq!(doc.active_page().height, 1080.0);
    }

    #[test]
    fn add_page_increases_count() {
        let mut doc = Document::new();
        let id = doc.add_page("Slide 2", 1280.0, 720.0);
        assert_eq!(doc.page_count(), 2);
        let page = doc.page(id).unwrap();
        assert_eq!(page.name, "Slide 2");
        assert_eq!(page.width, 1280.0);
    }

    #[test]
    fn remove_page_decreases_count() {
        let mut doc = Document::new();
        let id = doc.add_page("To Remove", 800.0, 600.0);
        assert_eq!(doc.page_count(), 2);
        assert!(doc.remove_page(id));
        assert_eq!(doc.page_count(), 1);
    }

    #[test]
    fn cannot_remove_last_page() {
        let mut doc = Document::new();
        let id = doc.active_page().id;
        assert!(!doc.remove_page(id));
        assert_eq!(doc.page_count(), 1);
    }

    #[test]
    fn remove_nonexistent_page_returns_false() {
        let mut doc = Document::new();
        assert!(!doc.remove_page(PageId::new()));
    }

    #[test]
    fn set_active_page_switches_page() {
        let mut doc = Document::new();
        let id2 = doc.add_page("Page 2", 800.0, 600.0);
        assert!(doc.set_active_page(id2));
        assert_eq!(doc.active_page().name, "Page 2");
        assert_eq!(doc.active_page_index(), 1);
    }

    #[test]
    fn set_active_page_nonexistent_returns_false() {
        let mut doc = Document::new();
        assert!(!doc.set_active_page(PageId::new()));
    }

    #[test]
    fn remove_active_page_adjusts_index() {
        let mut doc = Document::new();
        let _id2 = doc.add_page("Page 2", 800.0, 600.0);
        let id3 = doc.add_page("Page 3", 800.0, 600.0);
        doc.set_active_page(id3);
        assert_eq!(doc.active_page_index(), 2);
        doc.remove_page(id3);
        // Active index should clamp to last page.
        assert_eq!(doc.active_page_index(), 1);
    }

    #[test]
    fn page_mut_allows_modification() {
        let mut doc = Document::new();
        let id = doc.active_page().id;
        doc.page_mut(id).unwrap().name = "Renamed".to_string();
        assert_eq!(doc.active_page().name, "Renamed");
    }

    #[test]
    fn active_page_mut_allows_modification() {
        let mut doc = Document::new();
        doc.active_page_mut().name = "Modified".to_string();
        assert_eq!(doc.active_page().name, "Modified");
    }

    #[test]
    fn from_pages_creates_document() {
        let pages = vec![Page::new("A", 100.0, 100.0), Page::new("B", 200.0, 200.0)];
        let doc = Document::from_pages(pages);
        assert_eq!(doc.page_count(), 2);
        assert_eq!(doc.active_page().name, "A");
    }

    #[test]
    #[should_panic(expected = "must have at least one page")]
    fn from_pages_panics_on_empty() {
        let _ = Document::from_pages(vec![]);
    }

    #[test]
    fn pages_returns_all() {
        let mut doc = Document::new();
        doc.add_page("P2", 100.0, 100.0);
        assert_eq!(doc.pages().len(), 2);
    }

    #[test]
    fn add_page_with_id_inserts() {
        let mut doc = Document::new();
        let id = PageId::new();
        assert!(doc.add_page_with_id(id, "Custom", 800.0, 600.0));
        assert_eq!(doc.page_count(), 2);
        let page = doc.page(id).unwrap();
        assert_eq!(page.name, "Custom");
        assert_eq!(page.width, 800.0);
    }

    #[test]
    fn add_page_with_id_rejects_duplicate() {
        let mut doc = Document::new();
        let id = doc.active_page().id;
        assert!(!doc.add_page_with_id(id, "Dup", 100.0, 100.0));
        assert_eq!(doc.page_count(), 1);
    }
}
