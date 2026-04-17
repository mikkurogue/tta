pub mod type_checker;

use ariadne::{Cache, Color, Label, Report, ReportKind, Source};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Declaration, Statement};
use oxc_parser::Parser as OxcParser;
use oxc_span::SourceType;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use type_checker::FoundType;
use walkdir::WalkDir;

#[derive(clap::Parser)]
struct Cli {
    /// Path to .ts(x) file or directory
    path: Option<String>,

    /// Enable verbose logging for parse errors
    #[clap(short, long)]
    verbose: bool,

    /// Ignore warnings (only show critical/error diagnostics)
    #[clap(long)]
    ignore_warnings: bool,
}

fn parse_ts_code(
    code: &str,
    filename: &str,
    results: &mut HashMap<String, Vec<FoundType>>,
    verbose: bool,
) {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filename).unwrap_or_default();
    let parser_return = OxcParser::new(&allocator, code, source_type).parse();

    if !parser_return.errors.is_empty() && verbose {
        for error in &parser_return.errors {
            eprintln!("Parse error in {}: {}", filename, error);
        }
    }

    let program = parser_return.program;

    for stmt in &program.body {
        match stmt {
            Statement::TSTypeAliasDeclaration(type_alias) => {
                let found = FoundType::from_ast(type_alias, code, filename, false);
                results
                    .entry(found.name.clone())
                    .or_insert_with(Vec::new)
                    .push(found);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSTypeAliasDeclaration(type_alias)) = &export.declaration {
                    let found = FoundType::from_ast(type_alias, code, filename, true);
                    results
                        .entry(found.name.clone())
                        .or_insert_with(Vec::new)
                        .push(found);
                }
            }
            _ => {}
        }
    }
}

fn find_ts_files(path: &Path) -> Vec<String> {
    let mut ts_files = Vec::new();
    let excluded = [
        "node_modules",
        "dist",
        ".nx",
        "build",
        ".github",
        ".azuredevops",
        ".vscode",
        ".git",
        ".yarn",
        ".npm",
    ];

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !excluded.iter().any(|ex| name == *ex)
        })
        .filter_map(Result::ok)
    {
        if let Some(ext) = entry.path().extension() {
            if ext == "ts" || ext == "tsx" {
                ts_files.push(entry.path().to_string_lossy().to_string());
            }
        }
    }

    ts_files
}

/// Multi-file source cache for ariadne
struct FileCache {
    files: HashMap<String, Source<String>>,
}

impl FileCache {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    fn insert(&mut self, filename: String, source: String) {
        self.files.insert(filename, Source::from(source));
    }
}

#[allow(refining_impl_trait)]
impl Cache<String> for &FileCache {
    type Storage = String;

    fn fetch(&mut self, id: &String) -> Result<&Source<String>, Box<dyn fmt::Debug + '_>> {
        self.files
            .get(id)
            .ok_or_else(|| Box::new(format!("Unknown file: {}", id)) as Box<dyn fmt::Debug>)
    }

    fn display<'a>(&self, id: &'a String) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(id.clone()))
    }
}

fn main() {
    let args = Cli::parse();
    let target_path = args.path.unwrap_or_else(|| ".".to_string());
    let paths = find_ts_files(Path::new(&target_path));

    let mut results: HashMap<String, Vec<FoundType>> = HashMap::new();
    let mut source_cache = FileCache::new();

    let pb = ProgressBar::new(paths.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.yellow}] {pos}/{len} {eta}")
            .unwrap()
            .progress_chars("▇▆▅▄▃▂ "),
    );

    for path in &paths {
        let code = std::fs::read_to_string(path).expect("Failed to read source file");
        source_cache.insert(path.clone(), code.clone());
        parse_ts_code(&code, path, &mut results, args.verbose);
        pb.inc(1);
    }
    pb.finish_and_clear();

    eprintln!("Found {} unique TS type names.\n", results.len());

    let mut warning_count: usize = 0;
    let mut critical_count: usize = 0;

    for (type_name, types) in &results {
        if types.len() <= 1 {
            continue;
        }

        for i in 0..types.len() {
            for j in (i + 1)..types.len() {
                let type_a = &types[i];
                let type_b = &types[j];

                let is_critical = type_a.body == type_b.body;

                if !is_critical && args.ignore_warnings {
                    continue;
                }

                if is_critical {
                    critical_count += 1;

                    Report::build(
                        ReportKind::Error,
                        (type_a.filename.clone(), type_a.span_start..type_a.span_end),
                    )
                    .with_message(format!(
                        "Duplicate type '{}' with identical body",
                        type_name
                    ))
                    .with_label(
                        Label::new((type_a.filename.clone(), type_a.span_start..type_a.span_end))
                            .with_message("first defined here")
                            .with_color(Color::Red),
                    )
                    .with_label(
                        Label::new((type_b.filename.clone(), type_b.span_start..type_b.span_end))
                            .with_message("also defined here with the same body")
                            .with_color(Color::Red),
                    )
                    .with_note("Consider merging into a single shared type definition.")
                    .finish()
                    .eprint(&source_cache)
                    .unwrap();
                } else {
                    warning_count += 1;

                    Report::build(
                        ReportKind::Warning,
                        (type_a.filename.clone(), type_a.span_start..type_a.span_end),
                    )
                    .with_message(format!(
                        "Duplicate type name '{}' with different body",
                        type_name
                    ))
                    .with_label(
                        Label::new((type_a.filename.clone(), type_a.span_start..type_a.span_end))
                            .with_message("defined here")
                            .with_color(Color::Yellow),
                    )
                    .with_label(
                        Label::new((type_b.filename.clone(), type_b.span_start..type_b.span_end))
                            .with_message("also defined here with a different body")
                            .with_color(Color::Yellow),
                    )
                    .with_help(
                        "These types share a name but differ in structure. Consider renaming one.",
                    )
                    .finish()
                    .eprint(&source_cache)
                    .unwrap();
                }
            }
        }
    }

    eprintln!("\nWarnings: {}", warning_count);
    eprintln!("Critical: {}", critical_count);
}
