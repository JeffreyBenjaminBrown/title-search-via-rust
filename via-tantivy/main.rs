use tantivy::collector::TopDocs;
use tantivy::schema as schema;
use tantivy::{Index, doc};
use walkdir::WalkDir;
use regex::Regex;
use std::fs;

fn empty_temp_index(
    // Makes an empty index in a temporary directory.
    schema: schema::Schema,
    index_name: &str
) -> Result<Index, Box<dyn std::error::Error>> {
    let index_path = std::env::temp_dir().join(index_name);

    // TODO | PITFALL:
    // This deletes any existing data at that path!
    if index_path.exists() {
        fs::remove_dir_all(&index_path)?; }
    fs::create_dir_all(&index_path)?;
    println!("Creating index in {:?}", index_path);
    let index = Index::create_in_dir(
	&index_path, schema.clone())?;
    Ok(index)
}

fn build_index(
    index: &Index,
    path_field: schema::Field,
    title_field: schema::Field,
    data_dir: &str
) -> Result<usize, Box<dyn std::error::Error>> {
    println!("Indexing .org files in the {} directory...",
	     data_dir);
    let mut index_writer = index.writer(50_000_000)?;
    let mut indexed_count = 0;
    let title_re = Regex::new(
	r"(?i)^\s*#\+title:\s*(.*)$").unwrap();
    for entry in WalkDir::new(data_dir)
	.into_iter().filter_map(Result::ok)
    { let path = entry.path();
      if path.extension().map_or( // only process org files
	  false, |ext| ext == "org")
      { if let Ok(content) = std::fs::read_to_string(path)
	{ for line in content.lines()
	  { if let Some(cap) = title_re.captures(line)
	    { let title = cap[1].trim();
              println!("Indexing: {} - {}",
		       path.display(), title);
              index_writer.add_document(doc!(
                  path_field => path.to_string_lossy()
		      .to_string(),
                  title_field => title.to_string() ) )?;
              indexed_count += 1;
              break; } } } } }
    println!("Indexed {} files. Committing changes...",
	     indexed_count);
    index_writer.commit()?;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Index
    let mut schema_builder = schema::Schema::builder();
    let path_field = schema_builder.add_text_field(
        "path", schema::STRING | schema::STORED);
    let title_field = schema_builder.add_text_field(
        "title", schema::TEXT | schema::STORED);
    let schema = schema_builder.build();
    let index = // Later, `build_index` populates this.
	empty_temp_index( schema.clone(),
			  "tantivy_org_index")?;
    build_index(
	&index, path_field, title_field, "data")?;

    // Search
    let (best_matches, searcher) = search_index(
	&index, title_field, "test second")?;
    print_search_results( best_matches, &searcher,
			  path_field, title_field)?;
    Ok (()) }
