use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use regex::Regex;

type Index = HashMap<String, HashSet<PathBuf>>;

fn main() {
    let index = build_index("data");

    let query = "bears This";
    let results = search(&index, query);

    println!("Matches for '{}':", query);
    for path in results {
        println!("- {}", path.display());
    }
}

fn build_index(root: &str) -> Index {
    let mut index: Index = HashMap::new();

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if let Some((path, tokens)) = index_file(entry.path()) {
            for token in tokens {
                index.entry(token).or_default().insert(path.clone());
            }
        }
    }

    index
}

fn index_file(path: &Path) -> Option<(PathBuf, Vec<String>)> {
    if !path.is_file() || path.extension()?.to_str()? != "org" {
        return None;
    }

    let content = fs::read_to_string(path).ok()?;
    let title_line = content.lines().find(|l| l.starts_with("#+title:"))?;
    let raw_title = title_from_file(title_line)?;
    let cleaned = org_links_to_labels(&raw_title);
    let tokens = tokenize(&cleaned);

    Some((path.to_path_buf(), tokens))
}

// Returns files matching an unordered search for
// tokens in their titles.
fn search<'a>(
    index: &'a Index,
    query: &str,
) -> HashSet<&'a PathBuf> {
    let query_tokens = tokenize(query);
    let mut results: Option<HashSet<&PathBuf>> = None;

    for token in &query_tokens {
        if let Some(paths) = index.get(token) {
            results = Some(match results {
                Some(r) =>
		    r . intersection( &paths.iter() . collect() )
		    . cloned() . collect(),
                None => paths.iter().collect(),
            });
        } else {
            return HashSet::new();
        }
    }

    results.unwrap_or_default()
}

fn title_from_file(line: &str) -> Option<String> {
    if line.starts_with("#+title:") {
        Some(line.trim_start_matches("#+title:").trim().to_string())
    } else { None }
}

fn org_links_to_labels(text: &str) -> String {
    // Replaces any substring like `[[id][label]]` with `label`.
    let re = Regex::new (
	r"\[\[[^\[\]]+\]\[([^\[\]]+)\]\]" )
	. unwrap();
    re.replace_all(text, "$1").to_string()
}

fn tokenize(text: &str) -> Vec<String> {
    let re = Regex::new(r"\b\p{L}+\b").unwrap();  // match words
    re.find_iter(&text.to_lowercase())
        .map(|m| m.as_str().to_string())
        .collect()
}
