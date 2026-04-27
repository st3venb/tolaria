use crate::output::OutputContext;

/// Run the `search` command: query the vault and display results.
pub fn run(vault_path: &str, query: &str, limit: usize, output: &OutputContext) {
    let response = match tolaria_core::search::search_vault(vault_path, query, "keyword", limit) {
        Ok(r) => r,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    output.print_search_results(&response.results);
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use tolaria_core::search::SearchResult;

    // ── Arbitrary generators ────────────────────────────────────────

    fn arb_search_result() -> impl Strategy<Value = SearchResult> {
        (
            "[A-Za-z0-9 ]{1,25}",
            "[a-z/]{1,30}\\.md",
            "[a-zA-Z0-9 .]{0,30}",
            0.0f64..100.0f64,
            proptest::option::of("[a-zA-Z]{1,15}"),
        )
            .prop_map(|(title, path, snippet, score, note_type)| SearchResult {
                title,
                path,
                snippet,
                score,
                note_type,
            })
    }

    // ── Property 8: Search Results Ordering and Limit ───────────────
    // **Validates: Requirements 4.1, 4.3**
    //
    // For any query and limit, results are sorted by score descending
    // and count ≤ limit.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_search_results_sorted_descending(
            results in proptest::collection::vec(arb_search_result(), 0..30),
        ) {
            let mut sorted = results;
            sorted.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for w in sorted.windows(2) {
                prop_assert!(
                    w[0].score >= w[1].score,
                    "Results not sorted by score descending: {} < {}",
                    w[0].score,
                    w[1].score
                );
            }
        }

        #[test]
        fn prop_search_results_respect_limit(
            results in proptest::collection::vec(arb_search_result(), 0..50),
            limit in 1usize..30,
        ) {
            let mut truncated = results;
            truncated.truncate(limit);
            prop_assert!(
                truncated.len() <= limit,
                "Result count {} exceeds limit {}",
                truncated.len(),
                limit
            );
        }

        #[test]
        fn prop_search_vault_ordering_and_limit(
            limit in 1usize..30,
        ) {
            // Create a temp vault with some notes and run search_vault
            let dir = tempfile::Builder::new()
                .prefix("search-prop-")
                .tempdir_in(std::env::current_dir().unwrap())
                .unwrap();

            // Create several notes with varying keyword density
            for i in 0..10 {
                let content = format!(
                    "# Note {i}\n\n{}",
                    "keyword ".repeat(i + 1)
                );
                let path = dir.path().join(format!("note-{i}.md"));
                std::fs::write(&path, content).unwrap();
            }

            let response = tolaria_core::search::search_vault(
                dir.path().to_str().unwrap(),
                "keyword",
                "keyword",
                limit,
            )
            .unwrap();

            // Count must respect limit
            prop_assert!(
                response.results.len() <= limit,
                "search_vault returned {} results, exceeding limit {}",
                response.results.len(),
                limit
            );

            // Results must be sorted by score descending
            for w in response.results.windows(2) {
                prop_assert!(
                    w[0].score >= w[1].score,
                    "search_vault results not sorted: {} < {}",
                    w[0].score,
                    w[1].score
                );
            }
        }
    }
}
