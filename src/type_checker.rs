use oxc_ast::ast::{TSType, TSTypeAliasDeclaration};
use oxc_span::Span;

#[derive(Debug, Clone)]
pub struct FoundType {
    pub name: String,
    pub filename: String,
    pub line: usize,
    pub col: usize,
    pub span_start: usize,
    pub span_end: usize,
    pub is_exported: bool,
    pub body: String,
}

impl FoundType {
    pub fn from_ast(
        type_alias: &TSTypeAliasDeclaration,
        source: &str,
        filename: &str,
        is_exported: bool,
        override_span: Option<Span>,
    ) -> Self {
        let name = type_alias.id.name.to_string();

        let span = override_span.unwrap_or(type_alias.span);
        let start = span.start as usize;
        let end = span.end as usize;

        // Calculate line/col from byte offset
        let (line, col) = byte_offset_to_line_col(source, start);

        let body = serialize_ts_type(&type_alias.type_annotation);

        Self {
            name,
            body,
            filename: filename.to_string(),
            line,
            col,
            span_start: start,
            span_end: end,
            is_exported,
        }
    }
}

fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn serialize_ts_type(ts_type: &TSType) -> String {
    match ts_type {
        TSType::TSAnyKeyword(_) => "any".to_string(),
        TSType::TSBooleanKeyword(_) => "boolean".to_string(),
        TSType::TSNumberKeyword(_) => "number".to_string(),
        TSType::TSStringKeyword(_) => "string".to_string(),
        TSType::TSNullKeyword(_) => "null".to_string(),
        TSType::TSUndefinedKeyword(_) => "undefined".to_string(),
        TSType::TSVoidKeyword(_) => "void".to_string(),
        TSType::TSNeverKeyword(_) => "never".to_string(),
        TSType::TSUnknownKeyword(_) => "unknown".to_string(),
        TSType::TSBigIntKeyword(_) => "bigint".to_string(),
        TSType::TSSymbolKeyword(_) => "symbol".to_string(),
        TSType::TSObjectKeyword(_) => "object".to_string(),
        TSType::TSIntrinsicKeyword(_) => "intrinsic".to_string(),
        TSType::TSThisType(_) => "this".to_string(),

        TSType::TSTypeReference(r) => {
            let name = format!("{:?}", r.type_name);
            if let Some(params) = &r.type_arguments {
                let ps: Vec<String> = params.params.iter().map(|p| serialize_ts_type(p)).collect();
                format!("{}<{}>", name, ps.join(", "))
            } else {
                name
            }
        }

        TSType::TSTypeLiteral(lit) => {
            let members: Vec<String> = lit.members.iter().map(|m| format!("{:?}", m)).collect();
            format!("{{ {} }}", members.join("; "))
        }

        TSType::TSUnionType(u) => {
            let types: Vec<String> = u.types.iter().map(|t| serialize_ts_type(t)).collect();
            types.join(" | ")
        }

        TSType::TSIntersectionType(i) => {
            let types: Vec<String> = i.types.iter().map(|t| serialize_ts_type(t)).collect();
            types.join(" & ")
        }

        TSType::TSArrayType(a) => format!("{}[]", serialize_ts_type(&a.element_type)),

        TSType::TSTupleType(t) => {
            let elems: Vec<String> = t.element_types.iter().map(|e| format!("{:?}", e)).collect();
            format!("[{}]", elems.join(", "))
        }

        TSType::TSFunctionType(f) => format!("FunctionType({:?})", f.params),
        TSType::TSConstructorType(c) => format!("ConstructorType({:?})", c.params),
        TSType::TSConditionalType(c) => {
            format!(
                "{} extends {} ? {} : {}",
                serialize_ts_type(&c.check_type),
                serialize_ts_type(&c.extends_type),
                serialize_ts_type(&c.true_type),
                serialize_ts_type(&c.false_type)
            )
        }
        TSType::TSTypeQuery(q) => format!("typeof {:?}", q.expr_name),
        TSType::TSIndexedAccessType(i) => {
            format!(
                "{}[{}]",
                serialize_ts_type(&i.object_type),
                serialize_ts_type(&i.index_type)
            )
        }
        TSType::TSMappedType(m) => format!("MappedType({:?})", m.key),
        TSType::TSTypeOperatorType(o) => {
            format!("{:?} {}", o.operator, serialize_ts_type(&o.type_annotation))
        }
        TSType::TSImportType(i) => format!("import({:?})", i.source),
        TSType::TSParenthesizedType(p) => {
            format!("({})", serialize_ts_type(&p.type_annotation))
        }
        TSType::TSInferType(i) => format!("infer {:?}", i.type_parameter),
        TSType::TSLiteralType(l) => format!("{:?}", l.literal),
        TSType::TSTemplateLiteralType(t) => format!("TemplateLiteral({:?})", t.quasis),
        TSType::TSNamedTupleMember(m) => {
            format!("{}: {:?}", m.label, m.element_type)
        }
        TSType::JSDocNullableType(n) => format!("?{}", serialize_ts_type(&n.type_annotation)),
        TSType::JSDocNonNullableType(n) => format!("!{}", serialize_ts_type(&n.type_annotation)),
        TSType::JSDocUnknownType(_) => "unknown(jsdoc)".to_string(),
        TSType::TSTypePredicate(p) => format!("TypePredicate({:?})", p.parameter_name),
    }
}
