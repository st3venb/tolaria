use std::path::Path;

use tolaria_core::vault::create_note_content;

use crate::output::OutputContext;

/// Create a new markdown note at the vault root with the given title and optional type.
pub fn run(vault_path: &str, title: &str, note_type: Option<&str>, output: &OutputContext) {
    let filename = title_to_filename(title);
    let file_path = Path::new(vault_path).join(&filename);
    let file_path_str = file_path.to_string_lossy().to_string();

    let content = build_note_content(title, note_type);

    match create_note_content(&file_path_str, &content) {
        Ok(()) => {
            output.info(&format!("Created {}", filename));
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

/// Convert a title to a kebab-case filename with .md extension.
pub fn title_to_filename(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive dashes and trim leading/trailing dashes
    let collapsed = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    format!("{}.md", collapsed)
}

/// Build the markdown content for a new note.
pub fn build_note_content(title: &str, note_type: Option<&str>) -> String {
    let mut content = String::new();
    if let Some(t) = note_type {
        content.push_str("---\n");
        content.push_str(&format!("type: {}\n", t));
        content.push_str("---\n");
    }
    content.push_str(&format!("# {}\n", title));
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn title_to_filename_basic() {
        assert_eq!(title_to_filename("My New Note"), "my-new-note.md");
    }

    #[test]
    fn title_to_filename_special_chars() {
        let result = title_to_filename("Hello, World! (2025)");
        assert_eq!(result, "hello-world-2025.md");
        assert!(!result.contains("--"));
    }

    #[test]
    fn title_to_filename_already_kebab() {
        assert_eq!(title_to_filename("my-note"), "my-note.md");
    }

    #[test]
    fn build_content_without_type() {
        let content = build_note_content("Test Note", None);
        assert_eq!(content, "# Test Note\n");
        assert!(!content.contains("---"));
    }

    #[test]
    fn build_content_with_type() {
        let content = build_note_content("Test Note", Some("project"));
        assert!(content.contains("---\ntype: project\n---\n"));
        assert!(content.contains("# Test Note\n"));
    }

    #[test]
    fn build_content_h1_present() {
        let content = build_note_content("My Title", Some("note"));
        assert!(content.contains("# My Title"));
    }

    // ── Property 6: Note Creation Produces Valid File ────────────────
    // **Validates: Requirements 3.6, 3.7**
    //
    // For any valid title and optional type, created file has H1 heading
    // and correct frontmatter.

    fn arb_title() -> impl Strategy<Value = String> {
        "[A-Za-z][A-Za-z0-9 ]{0,30}"
            .prop_filter("non-empty title", |s| !s.trim().is_empty())
    }

    fn arb_opt_type() -> impl Strategy<Value = Option<String>> {
        proptest::option::of(prop_oneof![
            Just("project".to_string()),
            Just("note".to_string()),
            Just("area".to_string()),
            Just("person".to_string()),
            Just("topic".to_string()),
            Just("procedure".to_string()),
        ])
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_created_note_has_h1(title in arb_title(), note_type in arb_opt_type()) {
            let content = build_note_content(&title, note_type.as_deref());
            let h1_marker = format!("# {}", title);
            prop_assert!(
                content.contains(&h1_marker),
                "Created note missing H1 heading '{}' in:\n{}",
                h1_marker, content
            );
        }

        #[test]
        fn prop_created_note_frontmatter_type(title in arb_title(), note_type in arb_opt_type()) {
            let content = build_note_content(&title, note_type.as_deref());

            match note_type.as_deref() {
                Some(t) => {
                    // Must have frontmatter with type field
                    prop_assert!(content.starts_with("---\n"),
                        "Expected frontmatter when type is specified");
                    let type_line = format!("type: {}", t);
                    prop_assert!(content.contains(&type_line),
                        "Frontmatter missing '{}' in:\n{}", type_line, content);
                    // Frontmatter must be closed
                    let after_open = &content[4..]; // skip "---\n"
                    prop_assert!(after_open.contains("---\n"),
                        "Frontmatter not properly closed in:\n{}", content);
                }
                None => {
                    // No frontmatter delimiters
                    prop_assert!(!content.starts_with("---"),
                        "Unexpected frontmatter when no type specified:\n{}", content);
                }
            }
        }

        #[test]
        fn prop_created_note_filename_is_valid(title in arb_title()) {
            let filename = title_to_filename(&title);
            prop_assert!(filename.ends_with(".md"),
                "Filename must end with .md: {}", filename);
            prop_assert!(!filename.contains("--"),
                "Filename must not contain consecutive dashes: {}", filename);
            prop_assert!(!filename.starts_with('-'),
                "Filename must not start with dash: {}", filename);
            // Stem should not be empty
            let stem = &filename[..filename.len() - 3];
            prop_assert!(!stem.is_empty(),
                "Filename stem must not be empty");
        }
    }
}
