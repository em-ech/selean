//! Full project generation for a Vite + React + Tailwind CSS application.
//!
//! Given a [`Document`], generates a complete set of project files that can
//! be written to disk and run with `npm install && npm run dev`.

use selean_engine::persistence::Document;

use crate::jsx;

/// A single file in the generated project.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectFile {
    /// Relative path from the project root (e.g., `src/App.tsx`).
    pub path: String,
    /// File content.
    pub content: String,
}

/// The complete set of files for a generated project.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectOutput {
    /// All generated files.
    pub files: Vec<ProjectFile>,
}

/// Generates a complete Vite + React + Tailwind project from a [`Document`].
#[must_use]
pub fn generate_project(document: &Document) -> ProjectOutput {
    let mut files = vec![
        generate_package_json(),
        generate_vite_config(),
        generate_tailwind_config(),
        generate_index_css(),
        generate_index_html(),
        generate_main_tsx(),
    ];

    let component_names = generate_page_components(document, &mut files);
    files.push(generate_app_tsx(&component_names));

    ProjectOutput { files }
}

/// Generates a complete project with a CLAUDE.md file for Claude Code.
///
/// This is the same as [`generate_project`] but appends a CLAUDE.md containing
/// design tokens, component inventory, and project conventions.
#[must_use]
pub fn generate_project_with_claude_md(document: &Document) -> ProjectOutput {
    let mut output = generate_project(document);
    let claude_md = crate::claude_md::generate_claude_md(document);
    output.files.push(ProjectFile {
        path: "CLAUDE.md".to_string(),
        content: claude_md,
    });
    output
}

/// Generates one component file per page and returns their names.
fn generate_page_components(document: &Document, files: &mut Vec<ProjectFile>) -> Vec<String> {
    let mut names = Vec::new();
    for page in document.pages() {
        let name = jsx::sanitize_component_name(&page.name);
        let code = jsx::render_scene(&page.scene, &page.name, page.width, page.height);
        files.push(ProjectFile {
            path: format!("src/components/{name}.tsx"),
            content: code,
        });
        names.push(name);
    }
    names
}

/// Generates `package.json` with React, Vite, and Tailwind dependencies.
fn generate_package_json() -> ProjectFile {
    ProjectFile {
        path: "package.json".to_string(),
        content: r#"{
  "name": "selean-export",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.4.0",
    "tailwindcss": "^4.0.0",
    "typescript": "^5.7.0",
    "vite": "^6.0.0"
  }
}
"#
        .to_string(),
    }
}

/// Generates `vite.config.ts`.
fn generate_vite_config() -> ProjectFile {
    ProjectFile {
        path: "vite.config.ts".to_string(),
        content: r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
});
"#
        .to_string(),
    }
}

/// Generates a minimal `tailwind.config.js` (v4 uses CSS-based config).
fn generate_tailwind_config() -> ProjectFile {
    ProjectFile {
        path: "tailwind.config.js".to_string(),
        content: r#"/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {},
  },
  plugins: [],
};
"#
        .to_string(),
    }
}

/// Generates `src/index.css` with Tailwind directives.
fn generate_index_css() -> ProjectFile {
    ProjectFile {
        path: "src/index.css".to_string(),
        content: "@tailwind base;\n@tailwind components;\n@tailwind utilities;\n".to_string(),
    }
}

/// Generates `index.html` with the Vite entry point.
fn generate_index_html() -> ProjectFile {
    ProjectFile {
        path: "index.html".to_string(),
        content: r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Selean Export</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
        .to_string(),
    }
}

/// Generates `src/main.tsx` with the React root.
fn generate_main_tsx() -> ProjectFile {
    ProjectFile {
        path: "src/main.tsx".to_string(),
        content: r#"import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
"#
        .to_string(),
    }
}

/// Generates `src/App.tsx` that imports and renders all page components.
fn generate_app_tsx(component_names: &[String]) -> ProjectFile {
    let imports = component_names
        .iter()
        .map(|name| format!("import {name} from \"./components/{name}\";"))
        .collect::<Vec<_>>()
        .join("\n");

    let renders = if component_names.len() == 1 {
        format!("      <{} />", component_names[0])
    } else {
        component_names
            .iter()
            .map(|name| format!("      <{name} />"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let content = format!(
        "{imports}\n\n\
         export default function App() {{\n\
         {INDENT}return (\n\
         {INDENT}{INDENT}<div className=\"flex flex-col items-center gap-8 p-8\">\n\
         {renders}\n\
         {INDENT}{INDENT}</div>\n\
         {INDENT});\n\
         }}\n",
    );

    ProjectFile {
        path: "src/App.tsx".to_string(),
        content,
    }
}

/// Two-space indent constant.
const INDENT: &str = "  ";

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use selean_engine::persistence::Page;

    use super::*;

    #[test]
    fn single_page_file_list() {
        let doc = Document::new();
        let output = generate_project(&doc);
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"package.json"));
        assert!(paths.contains(&"vite.config.ts"));
        assert!(paths.contains(&"tailwind.config.js"));
        assert!(paths.contains(&"src/index.css"));
        assert!(paths.contains(&"index.html"));
        assert!(paths.contains(&"src/main.tsx"));
        assert!(paths.contains(&"src/App.tsx"));
        assert!(paths.contains(&"src/components/Page1.tsx"));
        assert_eq!(output.files.len(), 8);
    }

    #[test]
    fn multi_page_generates_one_component_per_page() {
        let pages = vec![
            Page::new("Slide 1", 1920.0, 1080.0),
            Page::new("Slide 2", 1920.0, 1080.0),
            Page::new("Slide 3", 1920.0, 1080.0),
        ];
        let doc = Document::from_pages(pages);
        let output = generate_project(&doc);
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"src/components/Slide1.tsx"));
        assert!(paths.contains(&"src/components/Slide2.tsx"));
        assert!(paths.contains(&"src/components/Slide3.tsx"));
        // 7 shared files + 3 components = 10
        assert_eq!(output.files.len(), 10);
    }

    #[test]
    fn package_json_is_valid_json() {
        let file = generate_package_json();
        let parsed: serde_json::Value = serde_json::from_str(&file.content).expect("valid JSON");
        assert!(parsed.get("dependencies").is_some());
        assert!(parsed.get("devDependencies").is_some());
    }

    #[test]
    fn index_html_has_root_div() {
        let file = generate_index_html();
        assert!(file.content.contains("id=\"root\""));
    }

    #[test]
    fn app_tsx_imports_all_pages() {
        let pages = vec![
            Page::new("Home", 800.0, 600.0),
            Page::new("About", 800.0, 600.0),
        ];
        let doc = Document::from_pages(pages);
        let output = generate_project(&doc);
        let app = output
            .files
            .iter()
            .find(|f| f.path == "src/App.tsx")
            .expect("App.tsx should exist");
        assert!(app.content.contains("import Home from"));
        assert!(app.content.contains("import About from"));
        assert!(app.content.contains("<Home />"));
        assert!(app.content.contains("<About />"));
    }

    #[test]
    fn tailwind_css_has_directives() {
        let file = generate_index_css();
        assert!(file.content.contains("@tailwind base"));
        assert!(file.content.contains("@tailwind utilities"));
    }

    #[test]
    fn main_tsx_has_strict_mode() {
        let file = generate_main_tsx();
        assert!(file.content.contains("StrictMode"));
        assert!(file.content.contains("createRoot"));
    }
}
