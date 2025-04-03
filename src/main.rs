pub mod type_checker;

use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::Path;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::{Decl, Module, ModuleItem, Stmt};
use swc_ecma_parser::TsSyntax;
use swc_ecma_parser::{lexer::Lexer, StringInput, Syntax};
use type_checker::FoundType;
use walkdir::WalkDir;

#[derive(clap::Parser)]
struct Cli {
    /// Path to .ts(x) file
    path: Option<String>,

    /// Enable verbose logging for errors
    #[clap(short, long)]
    verbose: bool,
}

fn parse_ts_code(
    code: &str,
    filename: &str,
    results: &mut HashMap<String, Vec<FoundType>>,
    verbose: bool,
) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(Lrc::new(FileName::Real(filename.into())), code.into());

    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: true,
            ..Default::default()
        }),
        Default::default(),
        StringInput::from(&*fm),
        None,
    );

    let mut parser = swc_ecma_parser::Parser::new_from(lexer);
    let module = match parser.parse_module() {
        Ok(module) => module,
        Err(err) => {
            if verbose {
                eprintln!(
                    "Error parsing {}: {:?}",
                    filename.red().bold().italic(),
                    err
                );
            }
            return;
        }
    };

    let mut type_list = Vec::new();
    extract_types(&module, &cm, &fm, filename, &mut type_list);

    for found_type in &type_list {
        results
            .entry(found_type.name.clone())
            .or_insert_with(Vec::new)
            .push(found_type.clone());
    }
}

fn extract_types(
    module: &Module,
    cm: &Lrc<SourceMap>,
    fm: &Lrc<swc_common::SourceFile>,
    filename: &str,
    list: &mut Vec<FoundType>,
) {
    for item in &module.body {
        if let ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(type_alias))) = item {
            list.push(FoundType::from_ast(type_alias, cm, fm, filename));
        }
    }
}

fn find_ts_files(path: &Path) -> Vec<String> {
    let mut ts_files = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            !e.path().to_string_lossy().contains("node_modules")
                || !e.path().to_string_lossy().contains("dist")
                || !e.path().to_string_lossy().contains(".nx")
                || !e.path().to_string_lossy().contains("build")
                || !e.path().to_string_lossy().contains(".github")
                || !e.path().to_string_lossy().contains(".azuredevops")
                || !e.path().to_string_lossy().contains(".vscode")
                || !e.path().to_string_lossy().contains(".git")
                || !e.path().to_string_lossy().contains(".yarn")
                || !e.path().to_string_lossy().contains(".npm")
        })
    // Explicitly filter out node_modules
    {
        if let Some(ext) = entry.path().extension() {
            if ext == "ts" || ext == "tsx" {
                ts_files.push(entry.path().to_string_lossy().to_string());
            }
        }
    }

    ts_files
}

fn main() {
    let args = Cli::parse();
    let target_path = args.path.unwrap_or_else(|| ".".to_string());
    let paths = find_ts_files(Path::new(&target_path));

    let mut results = HashMap::new();
    let pb = ProgressBar::new(paths.len() as u64);

    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.yellow}] {pos}/{len} {eta}")
            .unwrap()
            .progress_chars("▇▆▅▄▃▂ "),
    );

    for path in paths {
        let code = std::fs::read_to_string(&path).expect("Failed to read source file");
        parse_ts_code(&code, &path, &mut results, args.verbose);

        pb.inc(1);
    }
    println!(
        "\n{} {} unique TS type names.",
        "Found".green().bold(),
        results.len()
    );

    let mut warning_counter: usize = 0;
    let mut critical_counter: usize = 0;

    // Compare bodies of duplicate types
    for (type_name, types) in &results {
        if types.len() > 1 {
            // Compare each type with every other type
            for i in 0..types.len() {
                for j in (i + 1)..types.len() {
                    let type_a = &types[i];
                    let type_b = &types[j];

                    if type_a.body == type_b.body {
                        println!(
                            "{}\n{}",
                            "============================================"
                                .bright_blue()
                                .bold(),
                          format!(
                                "{} '{}' in '{}' declared at line {} has the same signature and body as '{}' in '{}' declared at line {}. Consider merging this to one type definition.",
                                "CRITICAL:".red().bold(),
                                type_name,
                                type_a.filename,
                                type_a.line,
                                type_name,
                                type_b.filename,
                                type_b.line
                            )
                            .red()
                            .bold()
                        );
                        println!(
                            "{}",
                            "============================================"
                                .bright_blue()
                                .bold()
                        );
                        critical_counter += 1;
                    } else {
                        println!(
                            "{}\n{}",
                            "============================================"
                                .bright_blue()
                                .bold(),
                            format!(
                                "{} '{}' in '{}' declared at line {} has the same name but a different body as '{}' in '{}' declared at line {}.",
                                "WARNING:".yellow().bold(),
                                type_name,
                                type_a.filename,
                                type_a.line,
                                type_name,
                                type_b.filename,
                                type_b.line
                            )
                            .yellow()
                            .bold()
                        );
                        println!(
                            "{}",
                            "============================================"
                                .bright_blue()
                                .bold()
                        );

                        warning_counter += 1
                    }
                }
            }
        }
    }

    println!("Warnings: {}", warning_counter);
    println!("Critical issues: {}", critical_counter);
    pb.finish_and_clear();
}
