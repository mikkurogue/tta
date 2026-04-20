pub mod interface_checker;
pub mod shared_lib;
pub mod type_checker;

use ariadne::{Cache, Color, Label, Report, ReportKind, Source};
use clap::Parser;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use oxc::allocator::Allocator;
use oxc::ast::ast::{Declaration, Statement, TSTypeName};
use oxc::parser::Parser as OxcParser;
use oxc::span::SourceType;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::interface_checker::InterfaceChecker;
use crate::shared_lib::{DeclarationChecker, FoundDeclarationNode};
use crate::type_checker::TypeChecker;

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
    results: &mut HashMap<String, Vec<FoundDeclarationNode>>,
    impl_counts: &mut HashMap<String, usize>,
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
                let checker = TypeChecker { type_alias };
                let found = checker.from_ast(code, filename, false, None);
                results
                    .entry(found.name.clone())
                    .or_insert_with(Vec::new)
                    .push(found);
            }
            Statement::TSInterfaceDeclaration(interface_decl) => {
                let checker = InterfaceChecker { interface_decl };
                let found = checker.from_ast(code, filename, false, None);
                results
                    .entry(found.name.clone())
                    .or_insert_with(Vec::new)
                    .push(found);
            }
            Statement::ClassDeclaration(class) => {
                for imp in &class.implements {
                    if let TSTypeName::IdentifierReference(id) = &imp.expression {
                        *impl_counts.entry(id.name.to_string()).or_insert(0) += 1;
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    match decl {
                        Declaration::TSTypeAliasDeclaration(type_alias) => {
                            let checker = TypeChecker { type_alias };
                            let found = checker.from_ast(code, filename, true, Some(export.span));
                            results
                                .entry(found.name.clone())
                                .or_insert_with(Vec::new)
                                .push(found);
                        }
                        Declaration::TSInterfaceDeclaration(interface_decl) => {
                            let checker = InterfaceChecker { interface_decl };
                            let found = checker.from_ast(code, filename, true, Some(export.span));
                            results
                                .entry(found.name.clone())
                                .or_insert_with(Vec::new)
                                .push(found);
                        }
                        Declaration::ClassDeclaration(class) => {
                            for imp in &class.implements {
                                if let TSTypeName::IdentifierReference(id) = &imp.expression {
                                    *impl_counts.entry(id.name.to_string()).or_insert(0) += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn find_ts_files(path: &Path) -> Vec<String> {
    let mut ts_files = Vec::new();

    for entry in WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
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

    let mut results: HashMap<String, Vec<FoundDeclarationNode>> = HashMap::new();
    let mut impl_counts: HashMap<String, usize> = HashMap::new();
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
        parse_ts_code(&code, path, &mut results, &mut impl_counts, args.verbose);
        pb.inc(1);
    }
    pb.finish_and_clear();

    eprintln!("Found {} unique TS type/interface names.\n", results.len());

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

                let kind_label = match (&type_a.ast_node_variant, &type_b.ast_node_variant) {
                    (shared_lib::AstNodeVariant::Type, shared_lib::AstNodeVariant::Type) => "type",
                    (
                        shared_lib::AstNodeVariant::Interface,
                        shared_lib::AstNodeVariant::Interface,
                    ) => "interface",
                    _ => "declaration",
                };

                if is_critical {
                    critical_count += 1;

                    let mut report = Report::build(
                        ReportKind::Error,
                        (type_a.filename.clone(), type_a.span_start..type_a.span_end),
                    )
                    .with_message(format!(
                        "Duplicate {} '{}' with identical body",
                        kind_label, type_name
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
                    .with_note(format!(
                        "Consider merging into a single shared {} definition.",
                        kind_label
                    ));

                    if kind_label == "interface" {
                        let count = impl_counts.get(type_name.as_str()).copied().unwrap_or(0);
                        report = report.with_help(format!(
                            "Found {} class implementation{} of '{}'.",
                            count,
                            if count == 1 { "" } else { "s" },
                            type_name
                        ));
                    }

                    report.finish().eprint(&source_cache).unwrap();
                } else {
                    warning_count += 1;

                    let mut report = Report::build(
                        ReportKind::Warning,
                        (type_a.filename.clone(), type_a.span_start..type_a.span_end),
                    )
                    .with_message(format!(
                        "Duplicate {} name '{}' with different body",
                        kind_label, type_name
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
                    .with_help(format!(
                        "These {}s share a name but differ in structure. Consider renaming one.",
                        kind_label
                    ));

                    if kind_label == "interface" {
                        let count = impl_counts.get(type_name.as_str()).copied().unwrap_or(0);
                        report = report.with_note(format!(
                            "Found {} class implementation{} of '{}'.",
                            count,
                            if count == 1 { "" } else { "s" },
                            type_name
                        ));
                    }

                    report.finish().eprint(&source_cache).unwrap();
                }
            }
        }
    }

    eprintln!("\nWarnings: {}", warning_count);
    eprintln!("Critical: {}", critical_count);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_impl_count_single_class() {
        let code = r#"
            interface IFoo { x: string; }
            class Bar implements IFoo { x = "hi"; }
        "#;
        let mut results = HashMap::new();
        let mut impl_counts = HashMap::new();
        parse_ts_code(code, "test.ts", &mut results, &mut impl_counts, false);
        assert_eq!(impl_counts.get("IFoo").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_impl_count_multiple_classes() {
        let code = r#"
            interface IFoo { x: string; }
            class A implements IFoo { x = "a"; }
            class B implements IFoo { x = "b"; }
            class C implements IFoo { x = "c"; }
        "#;
        let mut results = HashMap::new();
        let mut impl_counts = HashMap::new();
        parse_ts_code(code, "test.ts", &mut results, &mut impl_counts, false);
        assert_eq!(impl_counts.get("IFoo").copied().unwrap_or(0), 3);
    }

    #[test]
    fn test_impl_count_no_implementations() {
        let code = r#"
            interface IFoo { x: string; }
        "#;
        let mut results = HashMap::new();
        let mut impl_counts = HashMap::new();
        parse_ts_code(code, "test.ts", &mut results, &mut impl_counts, false);
        assert_eq!(impl_counts.get("IFoo").copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_impl_count_exported_class() {
        let code = r#"
            interface IFoo { x: string; }
            export class Bar implements IFoo { x = "hi"; }
        "#;
        let mut results = HashMap::new();
        let mut impl_counts = HashMap::new();
        parse_ts_code(code, "test.ts", &mut results, &mut impl_counts, false);
        assert_eq!(impl_counts.get("IFoo").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_impl_count_multiple_interfaces() {
        let code = r#"
            interface IFoo { x: string; }
            interface IBar { y: number; }
            class A implements IFoo { x = "a"; }
            class B implements IBar { y = 1; }
            class C implements IFoo { x = "c"; }
        "#;
        let mut results = HashMap::new();
        let mut impl_counts = HashMap::new();
        parse_ts_code(code, "test.ts", &mut results, &mut impl_counts, false);
        assert_eq!(impl_counts.get("IFoo").copied().unwrap_or(0), 2);
        assert_eq!(impl_counts.get("IBar").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_impl_count_accumulates_across_files() {
        let mut results = HashMap::new();
        let mut impl_counts = HashMap::new();

        let code1 = r#"
            interface IFoo { x: string; }
            class A implements IFoo { x = "a"; }
        "#;
        parse_ts_code(code1, "file1.ts", &mut results, &mut impl_counts, false);

        let code2 = r#"
            class B implements IFoo { x = "b"; }
        "#;
        parse_ts_code(code2, "file2.ts", &mut results, &mut impl_counts, false);

        assert_eq!(impl_counts.get("IFoo").copied().unwrap_or(0), 2);
    }
}
