use clap::Parser;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::{Decl, Module, ModuleItem, Stmt};
use swc_ecma_parser::TsSyntax;
use swc_ecma_parser::{lexer::Lexer, StringInput, Syntax};

#[derive(clap::Parser)]
struct Cli {
    /// Path to .ts(x) file
    path: String,
}

fn parse_ts_code(code: &str) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Anon.into(), code.into());

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
    let module = parser.parse_module().expect("Failed to parser module");

    extract_types(&module);
}

fn extract_types(module: &Module) {
    for item in &module.body {
        if let ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(type_alias))) = item {
            println!("Found type: {}", type_alias.id.sym)
        }
    }
}

fn main() {
    let args = Cli::parse();
    let code = std::fs::read_to_string(args.path).expect("Failed to read source file");

    parse_ts_code(&code);
}
