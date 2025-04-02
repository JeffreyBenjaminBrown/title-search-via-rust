use tantivy::collector::TopDocs;
use tantivy::schema::*;
use tantivy::{Index, doc};
use walkdir::WalkDir;
use regex::Regex;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define schema: file path + title text
    let mut schema_builder = Schema::builder();
    let path_field = schema_builder.add_text_field("path", STRING | STORED);
    let title_field = schema_builder.add_text_field("title", TEXT | STORED);
    let schema = schema_builder.build();

    // Create a temporary index directory
    let index_path = std::env::temp_dir().join("tantivy_org_index");

    // Ensure the directory exists
    if index_path.exists() {
        fs::remove_dir_all(&index_path)?;
    }
    fs::create_dir_all(&index_path)?;

    println!("Creating index in {:?}", index_path);

    let index = Index::create_in_dir(&index_path, schema.clone())?;
    let mut index_writer = index.writer(50_000_000)?;

    // Regex to match the title line (case-insensitive)
    let title_re = Regex::new(r"(?i)^\s*#\+title:\s*(.*)$").unwrap();

    println!("Indexing .org files in the data/ directory...");
    let mut indexed_count = 0;

    for entry in WalkDir::new("data").into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "org") {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    if let Some(cap) = title_re.captures(line) {
                        let title = cap[1].trim();
                        println!("Indexing: {} - {}", path.display(), title);

                        // Properly handle the Result
                        index_writer.add_document(doc!(
                            path_field => path.to_string_lossy().to_string(),
                            title_field => title.to_string()
                        ))?;

                        indexed_count += 1;
                        break;
                    }
                }
            }
        }
    }

    println!("Indexed {} files. Committing changes...", indexed_count);
    index_writer.commit()?;

    // Search the index for files with "second" in the title
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let query_parser = tantivy::query::QueryParser::for_index(&index, vec![title_field]);
    let query = query_parser.parse_query("second")?;
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;

    println!("\nFiles with 'second' in title:");
    if top_docs.is_empty() {
        println!("No matches found.");
    } else {
        for (score, doc_address) in top_docs {
            let retrieved_doc = searcher.doc(doc_address)?;
            let path_value = retrieved_doc.get_first(path_field).unwrap().as_text().unwrap();
            let title_value = retrieved_doc.get_first(title_field).unwrap().as_text().unwrap();
            println!("- Score: {:.2} | {} ({})", score, path_value, title_value);
        }
    }

    Ok(())
}
