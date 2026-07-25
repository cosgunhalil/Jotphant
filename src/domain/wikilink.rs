//! Parsing of `[[wiki-link]]` targets out of note bodies.
//!
//! A link is written `[[Title]]`, referring to another note by its title. The parser is
//! pure text processing; resolving titles to notes is the app layer's job.

/// Extracts the distinct, trimmed link targets from `text`, in order of appearance.
///
/// Empty targets and unterminated `[[` are ignored.
#[must_use]
pub fn extract_links(text: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let target = after[..end].trim();
        if !target.is_empty() && !links.iter().any(|existing| existing == target) {
            links.push(target.to_owned());
        }
        rest = &after[end + 2..];
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_single_link() {
        assert_eq!(extract_links("see [[Foo]] here"), vec!["Foo".to_owned()]);
    }

    #[test]
    fn extracts_multiple_links_in_order() {
        assert_eq!(
            extract_links("[[Alpha]] then [[Beta]]"),
            vec!["Alpha".to_owned(), "Beta".to_owned()]
        );
    }

    #[test]
    fn trims_whitespace_inside_brackets() {
        assert_eq!(extract_links("[[  Foo Bar  ]]"), vec!["Foo Bar".to_owned()]);
    }

    #[test]
    fn ignores_empty_targets() {
        assert_eq!(extract_links("[[]] [[X]]"), vec!["X".to_owned()]);
    }

    #[test]
    fn ignores_unterminated_brackets() {
        assert!(extract_links("[[Foo").is_empty());
    }

    #[test]
    fn deduplicates_repeated_targets() {
        assert_eq!(extract_links("[[A]] and [[A]]"), vec!["A".to_owned()]);
    }

    #[test]
    fn returns_empty_when_there_are_no_links() {
        assert!(extract_links("plain text, no links").is_empty());
    }
}
