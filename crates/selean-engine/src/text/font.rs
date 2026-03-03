//! Font loading and metric extraction.
//!
//! `FontData` owns raw TTF/OTF bytes and provides on-demand access to a parsed
//! `ttf_parser::Face` for outline extraction and metric queries. This avoids
//! self-referential struct issues by creating the `Face` on each call.

use std::collections::HashMap;

use selean_common::error::EngineError;

/// A font identifier within the `FontRegistry`.
pub type FontId = u16;

/// The default font embedded in the binary (Inter Variable).
const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterVariable.ttf");

/// Parsed font data, owned for the lifetime of the engine.
///
/// Contains the raw font file bytes. A `ttf_parser::Face` is created on demand
/// via the `face()` method, which borrows from the internal byte buffer.
pub struct FontData {
    /// Raw font file bytes (TTF or OTF).
    raw: Vec<u8>,
    /// Face index within a font collection (0 for single-font files).
    face_index: u32,
    /// Font units per em (for scaling metrics to pixels).
    units_per_em: u16,
    /// Horizontal ascender in font units (positive, above baseline).
    ascender: i16,
    /// Horizontal descender in font units (negative, below baseline).
    descender: i16,
    /// Line gap in font units.
    line_gap: i16,
}

impl FontData {
    /// Loads font data from raw bytes.
    ///
    /// # Arguments
    /// * `data` — Raw TTF, OTF, or TTC file bytes.
    /// * `face_index` — Face index within a font collection (use 0 for single-font files).
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the font cannot be parsed.
    pub fn from_bytes(data: Vec<u8>, face_index: u32) -> Result<Self, EngineError> {
        let face = ttf_parser::Face::parse(&data, face_index).map_err(|e| EngineError::Font {
            reason: format!("failed to parse font: {e}"),
        })?;

        let units_per_em = face.units_per_em();
        let ascender = face.ascender();
        let descender = face.descender();
        let line_gap = face.line_gap();

        Ok(Self {
            raw: data,
            face_index,
            units_per_em,
            ascender,
            descender,
            line_gap,
        })
    }

    /// Loads the built-in default font (Inter Variable).
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the embedded font data is invalid.
    pub fn default_font() -> Result<Self, EngineError> {
        Self::from_bytes(DEFAULT_FONT_BYTES.to_vec(), 0)
    }

    /// Creates a `ttf_parser::Face` borrowing from the internal byte buffer.
    ///
    /// This is cheap (sub-microsecond) and avoids self-referential struct issues.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the face cannot be parsed.
    pub fn face(&self) -> Result<ttf_parser::Face<'_>, EngineError> {
        ttf_parser::Face::parse(&self.raw, self.face_index).map_err(|e| EngineError::Font {
            reason: format!("failed to create face: {e}"),
        })
    }

    /// Creates a `ttf_parser::Face` with the `wght` variation axis set.
    ///
    /// For variable fonts, this produces outlines at the requested weight.
    /// For static fonts, the variation is silently ignored.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the face cannot be parsed.
    pub fn face_with_weight(&self, weight: u16) -> Result<ttf_parser::Face<'_>, EngineError> {
        let mut face = self.face()?;
        face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), f32::from(weight));
        Ok(face)
    }

    /// Returns the font's units-per-em value.
    #[must_use]
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Returns the horizontal ascender in font units (positive, above baseline).
    #[must_use]
    pub fn ascender(&self) -> i16 {
        self.ascender
    }

    /// Returns the horizontal descender in font units (negative, below baseline).
    #[must_use]
    pub fn descender(&self) -> i16 {
        self.descender
    }

    /// Returns the line gap in font units.
    #[must_use]
    pub fn line_gap(&self) -> i16 {
        self.line_gap
    }

    /// Returns the raw font bytes (for passing to `rustybuzz`).
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Returns the face index.
    #[must_use]
    pub fn face_index(&self) -> u32 {
        self.face_index
    }
}

/// Registry of loaded fonts, keyed by family name.
///
/// Font ID 0 is always Inter (the default font). Additional fonts can be
/// registered at runtime. Unknown family names fall back to ID 0.
pub struct FontRegistry {
    fonts: Vec<FontData>,
    name_to_id: HashMap<String, FontId>,
}

impl FontRegistry {
    /// Creates a new registry with Inter as the default font (ID 0).
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the embedded Inter font cannot be parsed.
    pub fn new() -> Result<Self, EngineError> {
        let inter = FontData::default_font()?;
        let mut name_to_id = HashMap::new();
        name_to_id.insert("inter".to_string(), 0);
        Ok(Self {
            fonts: vec![inter],
            name_to_id,
        })
    }

    /// Registers a new font family.
    ///
    /// Returns the assigned `FontId`. If the family is already registered,
    /// returns the existing ID.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the font bytes cannot be parsed.
    pub fn register(&mut self, family: &str, data: Vec<u8>) -> Result<FontId, EngineError> {
        let key = family.to_lowercase();
        if let Some(&id) = self.name_to_id.get(&key) {
            return Ok(id);
        }

        let font_data = FontData::from_bytes(data, 0)?;

        #[allow(clippy::cast_possible_truncation)]
        let id = self.fonts.len() as FontId;
        self.fonts.push(font_data);
        self.name_to_id.insert(key, id);
        Ok(id)
    }

    /// Returns the `FontData` for a given ID.
    #[must_use]
    pub fn get(&self, id: FontId) -> Option<&FontData> {
        self.fonts.get(usize::from(id))
    }

    /// Resolves a family name to a `FontId`.
    ///
    /// Falls back to 0 (Inter) if the family is not registered.
    #[must_use]
    pub fn resolve(&self, family: &str) -> FontId {
        let key = family.to_lowercase();
        self.name_to_id.get(&key).copied().unwrap_or(0)
    }

    /// Returns the number of registered fonts.
    #[must_use]
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }
}

impl std::fmt::Debug for FontRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontRegistry")
            .field("font_count", &self.fonts.len())
            .field("families", &self.name_to_id.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl std::fmt::Debug for FontData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontData")
            .field("size_bytes", &self.raw.len())
            .field("face_index", &self.face_index)
            .field("units_per_em", &self.units_per_em)
            .field("ascender", &self.ascender)
            .field("descender", &self.descender)
            .field("line_gap", &self.line_gap)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_default_font() {
        let font = FontData::default_font();
        assert!(font.is_ok());

        let font = font.unwrap_or_else(|_| unreachable!());
        assert!(font.units_per_em() > 0);
        assert!(font.ascender() > 0);
        assert!(font.descender() < 0);
    }

    #[test]
    fn face_creation_succeeds() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face();
        assert!(face.is_ok());
    }

    #[test]
    fn metrics_are_consistent() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face().unwrap_or_else(|_| unreachable!());
        assert_eq!(font.units_per_em(), face.units_per_em());
        assert_eq!(font.ascender(), face.ascender());
        assert_eq!(font.descender(), face.descender());
    }

    #[test]
    fn invalid_font_data_returns_error() {
        let result = FontData::from_bytes(vec![0, 1, 2, 3], 0);
        assert!(result.is_err());
    }

    #[test]
    fn debug_format() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let debug_str = format!("{font:?}");
        assert!(debug_str.contains("FontData"));
        assert!(debug_str.contains("units_per_em"));
    }

    #[test]
    fn raw_bytes_matches_default() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        assert_eq!(font.raw_bytes().len(), DEFAULT_FONT_BYTES.len());
    }

    #[test]
    fn face_with_weight_succeeds() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face_with_weight(700);
        assert!(face.is_ok());
    }

    #[test]
    fn face_with_weight_regular_matches_default() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let default_face = font.face().unwrap_or_else(|_| unreachable!());
        let weighted_face = font
            .face_with_weight(400)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(default_face.units_per_em(), weighted_face.units_per_em());
        assert_eq!(default_face.ascender(), weighted_face.ascender());
    }

    #[test]
    fn face_with_weight_bold_succeeds() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        // Bold (700) should parse fine on a variable font.
        let face = font
            .face_with_weight(700)
            .unwrap_or_else(|_| unreachable!());
        assert!(face.units_per_em() > 0);
    }

    // --- FontRegistry tests ---

    #[test]
    fn registry_new_has_inter() {
        let reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        assert_eq!(reg.font_count(), 1);
        assert!(reg.get(0).is_some());
    }

    #[test]
    fn registry_resolve_inter_case_insensitive() {
        let reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        assert_eq!(reg.resolve("Inter"), 0);
        assert_eq!(reg.resolve("inter"), 0);
        assert_eq!(reg.resolve("INTER"), 0);
    }

    #[test]
    fn registry_resolve_unknown_falls_back() {
        let reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        assert_eq!(reg.resolve("Roboto"), 0);
        assert_eq!(reg.resolve("NonExistent"), 0);
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        // Register the same Inter font under a different name for testing.
        let data = DEFAULT_FONT_BYTES.to_vec();
        let id = reg
            .register("TestFont", data)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(id, 1);
        assert_eq!(reg.font_count(), 2);
        assert!(reg.get(id).is_some());
        assert_eq!(reg.resolve("testfont"), id);
    }

    #[test]
    fn registry_duplicate_register_returns_same_id() {
        let mut reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        let data1 = DEFAULT_FONT_BYTES.to_vec();
        let data2 = DEFAULT_FONT_BYTES.to_vec();
        let id1 = reg
            .register("DupFont", data1)
            .unwrap_or_else(|_| unreachable!());
        let id2 = reg
            .register("DupFont", data2)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(id1, id2);
        assert_eq!(reg.font_count(), 2);
    }

    #[test]
    fn registry_invalid_bytes_error() {
        let mut reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        let result = reg.register("BadFont", vec![0, 1, 2, 3]);
        assert!(result.is_err());
        assert_eq!(reg.font_count(), 1);
    }

    #[test]
    fn registry_get_out_of_bounds() {
        let reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        assert!(reg.get(99).is_none());
    }

    #[test]
    fn registry_debug_format() {
        let reg = FontRegistry::new().unwrap_or_else(|_| unreachable!());
        let debug_str = format!("{reg:?}");
        assert!(debug_str.contains("FontRegistry"));
        assert!(debug_str.contains("font_count"));
    }
}
