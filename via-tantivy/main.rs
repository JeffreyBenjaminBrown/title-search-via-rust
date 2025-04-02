use tantivy::collector::TopDocs;
use tantivy::schema as schema;
use tantivy::{Index, doc};
use walkdir::WalkDir;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Index
    let mut schema_builder = schema::Schema::builder();
    let path_field = schema_builder.add_text_field(
        "path", schema::STRING | schema::STORED);
    let title_field = schema_builder.add_text_field(
        "title", schema::TEXT | schema::STORED);
    let schema = schema_builder.build();

    // Create or open the index in data/index.tantivy
    let index_path = "data/index.tantivy";
    let index = get_or_create_index(schema.clone(), index_path)?;

    // Build/update the index
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
) -> Result<Index, Box<dyn std::error::Error>> {
    let path = Path::new(index_path);

    if path.exists() {
        println!("Opening existing index at {:?}", path);
        Ok(Index::open_in_dir(path)?)
    } else {
        println!("Creating new index at {:?}", path);
        fs::create_dir_all(path)?;
        Ok(Index::create_in_dir(path, schema)?)
    }
}

fn get_modification_time(
    path: &Path)
    -> Result<SystemTime, Box<dyn std::error::Error>> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.modified()?) }

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
    let title_re = Regex::new(r"(?i)^\s*#\+title:\s*(.*)$").unwrap();

    // Get index modification time (if it exists)
    let index_mtime =
	match get_modification_time(index_path)
    { Ok(time) => time,
      // Default to epoch if we can't get the time
      Err(_) => SystemTime::UNIX_EPOCH, };
    for entry in WalkDir::new(data_dir) // walk org files
	.into_iter().filter_map(Result::ok)
    { let path = entry.path();
      if !path.extension().map_or( // skip non-org files
	  false, |ext| ext == "org") {
          continue; }
      if path.starts_with(index_path) {
	  continue; } // Don't traverse inside the index

      let file_mtime = match get_modification_time(path) {
          Ok(mtime) => mtime,
          Err(_) => continue, // Skip files we can't get modification time for
      };

      if file_mtime <= index_mtime {
	  // Skip files older than the index
          println!("Skipping unchanged file: {}",
		   path.display());
          continue; }

      // Process file
      if let Ok(content) = std::fs::read_to_string(path)
      { for line in content.lines()
        { if let Some(cap) = title_re.captures(line)
          { let title = cap[1].trim();
            println!("Indexing: {} - {}",
		     path.display(), title);

            // Delete existing documents
	    // with this path from the index
            let path_str = path.to_string_lossy()
	    .to_string();
            let term = tantivy::Term::from_field_text(
		path_field, &path_str);
            index_writer.delete_term(term);

            index_writer.add_document(doc!(
                path_field => path_str,
                title_field => title.to_string()
            ))?;

            indexed_count += 1;
            break; } } } }

    if indexed_count > 0
    { println!("Indexed {} files. Committing changes...",
	       indexed_count);
      index_writer.commit()?;
    } else {
        println!("No new or modified files found."); }
    Ok(indexed_count) }

fn search_index(
    index: &Index,
    title_field: schema::Field,
    query_text: &str
) -> Result< (Vec<(f32, tantivy::DocAddress)>,
	      tantivy::Searcher),
	    Box<dyn std::error::Error>> {
    println!("\nFinding files with titles matching \"{}\".",
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
