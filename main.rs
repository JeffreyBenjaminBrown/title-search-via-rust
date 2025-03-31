use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;
use regex::Regex;

fn extract_title(line: &str) -> Option<String> {
    if line.starts_with("#+title:") {
        Some(line.trim_start_matches("#+title:").trim().to_string())
    } else { None }
}

fn clean_org_links(text: &str) -> String {
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

fn main() {
    let mut index: HashMap<String, HashSet<PathBuf>> = HashMap::new();

    for entry in WalkDir::new("data").into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("org") {
            if let Ok(content) = fs::read_to_string(path) {
                if let Some(title_line) = content.lines().find(|l| l.starts_with("#+title:")) {
                    if let Some(raw_title) = extract_title(title_line) {
                        let cleaned = clean_org_links(&raw_title);
                        let tokens = tokenize(&cleaned);

                        for token in tokens {
                            index.entry(token).or_default().insert(path.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    let query = "bears This";
    let query_tokens = tokenize(query);

    let mut results: Option<HashSet<_>> = None;

    for token in &query_tokens {
        if let Some(paths) = index.get(token) {
            results = Some(match results {
                Some(r) => r.intersection(paths).cloned().collect(),
                None => paths.clone(),
            });
        } else {
            results = Some(HashSet::new());
            break;
        }
    }

    println!("Matches for '{}':", query);
    if let Some(matches) = results {
        for path in matches {
            println!("- {}", path.display());
        }
    } else {
        println!("No matches.");
    }
}
