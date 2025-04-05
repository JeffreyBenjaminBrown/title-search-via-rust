use tantivy::collector::TopDocs;
use tantivy::schema as schema;
use tantivy::{Index, doc};
use walkdir::WalkDir;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define the schema
    let mut schema_builder = schema::Schema::builder();
    let path_field = schema_builder.add_text_field(
        "path", schema::STRING | schema::STORED);
    let title_field = schema_builder.add_text_field(
        "title", schema::TEXT | schema::STORED);
    let schema = schema_builder.build();

    // Build, or find and update, the index
    let index_path = "data/index.tantivy";
    let index = get_or_create_index(schema.clone(), index_path)?;
    update_index(&index, path_field,
		 title_field, "data",
		 Path::new(index_path))?;

    // Search
    let (best_matches, searcher) = search_index(
	&index, title_field, "test second")?;
    print_search_results( best_matches, &searcher,
			  path_field, title_field)?;
    Ok (()) }

fn get_or_create_index(
    schema: schema::Schema,
    index_path: &str
) -> Result<Index, Box<dyn std::error::Error>>
{ let path = Path::new(index_path);
  if path.exists() {
      println!("Opening existing index at {:?}", path);
      Ok(Index::open_in_dir(path)?)
  } else {
      println!("Creating new index at {:?}", path);
      fs::create_dir_all(path)?;
      Ok(Index::create_in_dir(path, schema)?) } }

fn get_modification_time(
    path: &Path)
    -> Result<SystemTime, Box<dyn std::error::Error>>
{ let metadata = fs::metadata(path)?;
  Ok(metadata.modified()?) }

fn needs_indexing( // based on modification time
    path: &Path,
    index_mtime: SystemTime
) -> bool {
    if path.to_string_lossy().contains("index.tantivy")
    { return false; } // Skip files in the index directory
    if !path.extension().map_or(false, |ext| ext == "org")
    { return false; } // Skip non-org files
    match get_modification_time(path)
    { Ok(file_mtime) => file_mtime > index_mtime,
      Err(_) => true // If modification time is unknown,
                     // assume it needs indexing.
    } }

fn extract_org_title(path: &Path) -> Option<String> {
    let title_re = Regex::new(r"(?i)^\s*#\+title:\s*(.*)$").unwrap();

    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            if let Some(cap) = title_re.captures(line) {
                let raw_title = cap[1].trim().to_string();
                return Some(strip_org_links(&raw_title)); } } }
    None }

// Titles can include links,
// but can be searched for as if each link
// was equal to its label.
// That is, the ID and brackets of a link in a title are not indexed.
fn strip_org_links(text: &str) -> String {
    let link_re = Regex::new(
	r"\[\[.*?\]\[(.*?)\]\]").unwrap();
    let mut result = String::from(text);
    let mut offset = 0; // This is offset in `text` -- that is, in the input string, not the output
    for cap in link_re.captures_iter(text) {
        let whole_match = cap.get(0).unwrap();
        let link_label = cap.get(1).unwrap();

        // Define the range to modify
        let start_pos = whole_match.start() - offset;
        let end_pos = whole_match.end() - offset;

        result.replace_range(
	    start_pos .. end_pos,
	    link_label.as_str());
        offset += whole_match.len() - link_label.len(); }
    result }

// Add a single document to the index
fn index_document(
    writer: &mut tantivy::IndexWriter,
    path: &Path,
    title: &str,
    path_field: schema::Field,
    title_field: schema::Field
) -> Result<(), Box<dyn std::error::Error>> {
    let path_str = path.to_string_lossy().to_string();
    let term = tantivy::Term::from_field_text(
	path_field, &path_str);
    writer.delete_term( // Delete (from the index)
                        // anything with this path.
	term);
    writer.add_document(doc!(
        path_field => path_str,
        title_field => title.to_string()
    ))?;
    Ok (()) }

fn update_index(
    index: &Index,
    path_field: schema::Field,
    title_field: schema::Field,
    data_dir: &str,
    index_path: &Path
) -> Result<usize, Box<dyn std::error::Error>> {
    println!("Updating index with .org files in the {} directory...", data_dir);
    let mut index_writer = index.writer(50_000_000)?;
    let mut indexed_count = 0;
    let index_mtime = get_modification_time(index_path)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for entry in WalkDir::new(data_dir)
	.into_iter().filter_map(Result::ok)
    { let path = entry.path();
      if !needs_indexing(path, index_mtime)
      { if path.extension().map_or(
	  false, |ext| ext == "org")
	{ println!("Skipping unchanged file: {}",
		   path.display()); }
        continue; }
      if let Some(title) = extract_org_title(path)
      { println!("Indexing: {} - {}",
		 path.display(), title);
        index_document(&mut index_writer, path, &title,
                       path_field, title_field)?;
        indexed_count += 1; } }
    if indexed_count > 0
    { println!("Indexed {} files. Committing changes...",
	       indexed_count);
      index_writer.commit()?;
    } else
    { println!("No new or modified files found."); }
    Ok(indexed_count) }

fn search_index(
    index: &Index,
    title_field: schema::Field,
    query_text: &str
) -> Result< (Vec<(f32, tantivy::DocAddress)>,
	      tantivy::Searcher),
	    Box<dyn std::error::Error>> {
    println!(
	"\nFinding files with titles matching \"{}\".",
	query_text);
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let query_parser =
	tantivy::query::QueryParser::for_index(
        &index, vec![title_field]);
    let query = query_parser.parse_query(query_text)?;
    let best_matches = searcher.search(
        &query, &TopDocs::with_limit(10))?;
    Ok((best_matches, searcher)) }

fn print_search_results(
    best_matches: Vec< // vector of float-address pairs
	    (f32, tantivy::DocAddress)>,
    searcher: &tantivy::Searcher,
    path_field: schema::Field,
    title_field: schema::Field
) -> Result<(), Box<dyn std::error::Error>> {
    if best_matches.is_empty() {
        println!("No matches found.");
    } else {
        for (score, doc_address) in best_matches {
            let retrieved_doc = searcher.doc(doc_address)?;
            let path_value =
                retrieved_doc.get_first(path_field)
                .unwrap().as_text().unwrap();
            let title_value =
                retrieved_doc.get_first(title_field)
                .unwrap().as_text().unwrap();
            println!("- Score: {:.2} | {} ({})",
                     score, path_value, title_value); } }
    Ok (()) }
