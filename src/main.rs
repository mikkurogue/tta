use clap::Parser;
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

#[derive(Debug)]
pub struct FoundType {
    pub name: String,
    pub line: usize,
    pub is_exported: bool,
}

impl FoundType {
    fn from_ast(
        type_alias: &TsTypeAliasDecl,
        cm: &Lrc<SourceMap>,
        fm: &Lrc<swc_common::SourceFile>,
    ) -> Self {
        let name = type_alias.id.sym.to_string();
        let line = cm
            .lookup_line(fm.start_pos + type_alias.span.lo)
            .map(|pos| pos.line + 1)
            .unwrap_or(0);

        let is_exported = matches!(type_alias.declare, true);

        Self {
            name,
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
    let module = parser.parse_module().expect("Failed to parse module");

    let mut type_list = Vec::new();
    extract_types(&module, &cm, &fm, &mut type_list);

    if !type_list.is_empty() {
        results.insert(filename.to_string(), type_list);
    }
}

fn extract_types(
    module: &Module,
    cm: &Lrc<SourceMap>,
    fm: &Lrc<swc_common::SourceFile>,
    list: &mut Vec<FoundType>,
) {
    for item in &module.body {
        if let ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(type_alias))) = item {
            list.push(FoundType::from_ast(type_alias, cm, fm));
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

    println!("\n Found {} files with TS types", results.len());

    for (file, types) in &results {
        println!("\n File: {}", file);

        for t in types {
            let export_status = if t.is_exported { "exported" } else { "local" };
            println!(" - {} (line {}, {})", t.name, t.line, export_status);
        }
    }
}
