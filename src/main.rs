use clap::Parser;
use colored::*;
use std::collections::HashMap;
use std::path::Path;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::{Decl, Module, ModuleItem, Stmt, TsTypeAliasDecl};
use swc_ecma_parser::TsSyntax;
use swc_ecma_parser::{lexer::Lexer, StringInput, Syntax};
use walkdir::WalkDir;

#[derive(clap::Parser)]
struct Cli {
    /// Path to .ts(x) file
    path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FoundType {
    pub name: String,
    pub filename: String,
    pub line: usize,
    pub is_exported: bool,
}

impl FoundType {
    fn from_ast(
        type_alias: &TsTypeAliasDecl,
        cm: &Lrc<SourceMap>,
        fm: &Lrc<swc_common::SourceFile>,
        filename: &str,
    ) -> Self {
        let name = type_alias.id.sym.to_string();
        let line = cm
            .lookup_line(fm.start_pos + type_alias.span.lo)
            .map(|pos| pos.line + 1)
            .unwrap_or(0);

        let is_exported = matches!(type_alias.declare, true);

        Self {
            name,
            filename: filename.to_string(),
            line,
            is_exported,
        }
    }
}

fn parse_ts_code(code: &str, filename: &str, results: &mut HashMap<String, Vec<FoundType>>) {
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
            eprintln!("Error parsing {}: {:?}", filename, err);
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
    let mut files = Vec::new();
    if path.is_file() {
        if let Some(ext) = path.extension() {
            if ext == "ts" || ext == "tsx" {
                files.push(path.to_string_lossy().into_owned());
            }
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
        {
            if let Some(ext) = entry.path().extension() {
                if ext == "ts" || ext == "tsx" {
                    files.push(entry.path().to_string_lossy().into_owned());
                }
            }
        }
    }
    files
}

fn main() {
    let args = Cli::parse();
    let target_path = args.path.unwrap_or_else(|| ".".to_string());
    let paths = find_ts_files(Path::new(&target_path));

    let mut results = HashMap::new();
    for path in paths {
        let code = std::fs::read_to_string(&path).expect("Failed to read source file");
        parse_ts_code(&code, &path, &mut results);
    }
    println!(
        "\n{} {} unique TS type names.",
        "Found".green().bold(),
        results.len()
    );

    for (type_name, types) in &results {
        if types.len() > 1 {
            println!(
                "{}\n{}",
                "============================================"
                    .bright_blue()
                    .bold(),
                format!("{} '{}':", "WARNING: Possible duplicate type", type_name)
                    .yellow()
                    .bold()
            );
            for t in types {
                println!(
                    "  {} {}\n      {} {} {}",
                    "→ Found in".cyan().bold(),
                    t.filename,
                    "Line:".magenta().bold(),
                    t.line,
                    "--------------------------------------------".bright_black()
                );
            }
            println!(
                "{}",
                "============================================"
                    .bright_blue()
                    .bold()
            );
        }
    }
}
