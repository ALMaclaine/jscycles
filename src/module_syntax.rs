//! Native syntax traversal shared by extraction and its semantic fuzz harness.
use std::path::Path;
use std::sync::OnceLock;

use tree_sitter::{Node, Parser};

use crate::ExtractError;
use crate::budget::{AnalysisBudget, Resource};

#[derive(Clone, Copy)]
pub(crate) enum Language {
    JavaScript,
    TypeScript,
    Tsx,
}

impl Language {
    pub(crate) fn for_path(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("tsx") => Self::Tsx,
            Some("js" | "jsx" | "mjs" | "cjs") => Self::JavaScript,
            _ => Self::TypeScript,
        }
    }
}

impl Language {
    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }

    fn roles(self) -> &'static GrammarRoles {
        static JAVASCRIPT: OnceLock<GrammarRoles> = OnceLock::new();
        static TYPESCRIPT: OnceLock<GrammarRoles> = OnceLock::new();
        static TSX: OnceLock<GrammarRoles> = OnceLock::new();
        let cache = match self {
            Self::JavaScript => &JAVASCRIPT,
            Self::TypeScript => &TYPESCRIPT,
            Self::Tsx => &TSX,
        };
        cache.get_or_init(|| GrammarRoles::new(&self.grammar()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SyntaxKind {
    Other,
    SourceStatement,
    Call,
    Import,
    Identifier,
    String,
    IgnoredExpression,
    WrappedExpression,
}

/// Each pinned grammar supplies its own symbol IDs, including aliases with the
/// same name. Cache only immutable classifications, never trees or source data.
struct GrammarRoles {
    kinds: Vec<SyntaxKind>,
}

impl GrammarRoles {
    fn new(grammar: &tree_sitter::Language) -> Self {
        let kinds = (0..grammar.node_kind_count())
            .map(|id| {
                match u16::try_from(id)
                    .ok()
                    .and_then(|id| grammar.node_kind_for_id(id))
                {
                    Some("import_statement" | "export_statement" | "import_require_clause") => {
                        SyntaxKind::SourceStatement
                    },
                    Some("call_expression") => SyntaxKind::Call,
                    Some("import") => SyntaxKind::Import,
                    Some("identifier") => SyntaxKind::Identifier,
                    Some("string") => SyntaxKind::String,
                    Some("comment" | "type_arguments") => SyntaxKind::IgnoredExpression,
                    Some(
                        "parenthesized_expression"
                        | "as_expression"
                        | "satisfies_expression"
                        | "non_null_expression"
                        | "type_assertion"
                        | "instantiation_expression",
                    ) => SyntaxKind::WrappedExpression,
                    _ => SyntaxKind::Other,
                }
            })
            .collect();
        Self { kinds }
    }

    fn kind(&self, node: &SyntaxNode<'_>) -> SyntaxKind {
        self.kinds
            .get(usize::from(node.kind_id()))
            .copied()
            .unwrap_or(SyntaxKind::Other)
    }
}

type SyntaxNode<'a> = Node<'a>;

fn text<'a>(node: &SyntaxNode<'_>, source: &'a str) -> Option<&'a str> {
    source.get(node.start_byte()..node.end_byte())
}

fn required_text<'a>(
    node: &SyntaxNode<'_>,
    source: &'a str,
    path: &Path,
) -> Result<&'a str, ExtractError> {
    text(node, source).ok_or_else(|| ExtractError::ParseError {
        path: path.to_path_buf(),
        message: "native syntax node has an invalid source range".to_owned(),
    })
}

fn native_length(length: usize, path: &Path) -> Result<u32, ExtractError> {
    u32::try_from(length).map_err(|error| ExtractError::ParseError {
        path: path.to_path_buf(),
        message: format!("source exceeds the native parser's byte range ({length} bytes): {error}"),
    })
}

/// Locate the first ERROR or MISSING node in document order for diagnostics:
/// 1-based line and byte column. Descends iteratively through children that
/// contain an error; the depth is bounded by the already-built tree.
fn first_error(root: &Node<'_>) -> (u64, u64, &'static str) {
    // `has_error` holds for ERROR and MISSING nodes themselves. Stop at an
    // ERROR node (its start is the earliest point); a MISSING token is a leaf.
    let mut node = *root;
    while !node.is_error() {
        let Some(child) = node.children(&mut node.walk()).find(Node::has_error) else {
            break;
        };
        node = child;
    }
    let kind = if node.is_missing() {
        "missing token"
    } else {
        "syntax error"
    };
    let position = node.start_position();
    (
        u64::try_from(position.row)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
        u64::try_from(position.column)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
        kind,
    )
}

/// Parse once, reject recovery, and admit source bytes before traversal. The sink
/// owns decoding, target resolution and admission of retained import records.
///
/// With `skip_type_only`, imports TypeScript erases from JavaScript output are
/// not reported: `import type`/`export type` statements (including
/// `import type x = require()`) and `import()` in type positions. A named
/// import whose specifiers are all `type` is still reported, because it stays
/// a side-effect import under `verbatimModuleSyntax`.
pub(crate) fn visit(
    source: &str,
    path: &Path,
    language: Language,
    budget: &AnalysisBudget,
    skip_type_only: bool,
    mut sink: impl FnMut(&str) -> Result<(), ExtractError>,
) -> Result<(), ExtractError> {
    budget.check()?;
    // The provider narrows its input length to u32 internally. Reject an
    // unrepresentable input rather than parsing a truncated prefix when an
    // operator deliberately raises the normal file/aggregate/work quotas.
    let _native_length = native_length(source.len(), path)?;
    // Charge source-dependent work before native parsing and the linear walk,
    // without contending on a shared atomic for every individual syntax node.
    budget.consume(
        Resource::Work,
        u64::try_from(source.len()).unwrap_or(u64::MAX).max(1),
    )?;
    let failure = |message| ExtractError::ParseError {
        path: path.to_path_buf(),
        message,
    };
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .map_err(|error| failure(format!("native parser failed: {error}")))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| failure("native syntax parser unavailable".to_owned()))?;
    let root = tree.root_node();
    if root.has_error() {
        let (line, column, kind) = first_error(&root);
        return Err(ExtractError::ParseError {
            path: path.to_path_buf(),
            message: format!(
                "source contains syntax errors or missing tokens (first {kind} at line {line}, column {column})"
            ),
        });
    }
    let roles = language.roles();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        let _ = required_text(&node, source, path)?;
        let literal = match roles.kind(&node) {
            SyntaxKind::SourceStatement if skip_type_only && is_type_only_statement(&node) => None,
            SyntaxKind::SourceStatement => node.child_by_field_name("source"),
            SyntaxKind::Call => call_source(&node, source, path, roles)?
                .filter(|_| !(skip_type_only && in_type_position(&node))),
            SyntaxKind::Other
            | SyntaxKind::Import
            | SyntaxKind::Identifier
            | SyntaxKind::String
            | SyntaxKind::IgnoredExpression
            | SyntaxKind::WrappedExpression => None,
        };
        if let Some(literal) = literal {
            sink(required_text(&literal, source, path)?)?;
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                budget.check()?;
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod length_tests {
    use super::native_length;
    use std::path::Path;

    #[test]
    fn native_byte_range_accepts_its_last_value_and_rejects_the_next() {
        let path = Path::new("source.ts");
        for length in [0, 1, 32768, u64::from(u32::MAX)] {
            if let Ok(length) = usize::try_from(length) {
                assert_eq!(
                    u64::from(native_length(length, path).expect("representable input")),
                    u64::try_from(length).expect("test length"),
                    "exact native range"
                );
            }
        }
        if let Ok(length) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert!(
                native_length(length, path).is_err(),
                "never truncate a length at the FFI boundary"
            );
        }
    }
}

/// `import type ...`, `export type ... from`, and `import type x = require()`
/// carry a `type` keyword directly after `import`/`export`.
fn is_type_only_statement(node: &SyntaxNode<'_>) -> bool {
    let mut cursor = node.walk();
    let mut children = node.children(&mut cursor);
    let first = children.next();
    let second = children.next();
    // The `import_require_clause` of `import type x = require()` is itself a
    // source statement; its parent statement holds the keyword.
    if node.kind() == "import_require_clause" {
        return node
            .parent()
            .is_some_and(|parent| is_type_only_statement(&parent));
    }
    first.is_some_and(|first| matches!(first.kind(), "import" | "export"))
        && second.is_some_and(|second| second.kind() == "type")
}

/// Whether a call lies in a type: an annotation, alias, interface, `typeof`
/// query, type arguments/parameters or heritage clause, or the type side of
/// `as`/`satisfies`. Called only for module calls when skipping is enabled.
fn in_type_position(node: &SyntaxNode<'_>) -> bool {
    let mut child = *node;
    while let Some(parent) = child.parent() {
        match parent.kind() {
            "type_annotation"
            | "opting_type_annotation"
            | "omitting_type_annotation"
            | "asserts_annotation"
            | "type_predicate_annotation"
            | "type_alias_declaration"
            | "interface_declaration"
            | "type_query"
            | "type_arguments"
            | "type_parameters"
            | "implements_clause"
            | "extends_type_clause" => return true,
            "as_expression" | "satisfies_expression"
                if parent.named_child(0).is_some_and(|value| value != child) =>
            {
                return true;
            },
            _ => {},
        }
        child = parent;
    }
    false
}

fn call_source<'a>(
    node: &SyntaxNode<'a>,
    source: &str,
    path: &Path,
    roles: &GrammarRoles,
) -> Result<Option<SyntaxNode<'a>>, ExtractError> {
    let Some(function) = node
        .child_by_field_name("function")
        .and_then(|function| unwrapped_expression(function, roles))
    else {
        return Ok(None);
    };
    let supported = match roles.kind(&function) {
        SyntaxKind::Import => true,
        SyntaxKind::Identifier => {
            crate::syntax::is_require_identifier(required_text(&function, source, path)?)
        },
        SyntaxKind::Other
        | SyntaxKind::SourceStatement
        | SyntaxKind::Call
        | SyntaxKind::String
        | SyntaxKind::IgnoredExpression
        | SyntaxKind::WrappedExpression => false,
    };
    if !supported {
        return Ok(None);
    }
    Ok(node
        .child_by_field_name("arguments")
        .and_then(|arguments| first_expression(&arguments, roles))
        .and_then(|first| unwrapped_expression(first, roles))
        .filter(|first| roles.kind(first) == SyntaxKind::String))
}

fn first_expression<'a>(node: &SyntaxNode<'a>, roles: &GrammarRoles) -> Option<SyntaxNode<'a>> {
    {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return None;
        }
        loop {
            let child = cursor.node();
            // A TypeScript angle assertion puts its type_arguments before the
            // value. Types and comments cannot supply a call's runtime value.
            if child.is_named() && roles.kind(&child) != SyntaxKind::IgnoredExpression {
                return Some(child);
            }
            if !cursor.goto_next_sibling() {
                return None;
            }
        }
    }
}

fn unwrapped_expression<'a>(
    mut node: SyntaxNode<'a>,
    roles: &GrammarRoles,
) -> Option<SyntaxNode<'a>> {
    while roles.kind(&node) == SyntaxKind::WrappedExpression {
        node = first_expression(&node, roles)?;
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::AnalysisLimits;

    #[test]
    fn first_error_reports_the_earliest_error_or_missing_token() {
        for (source, expected) in [
            ("let a = 1;\nlet b = (2;\n", (2, 11, "missing token")),
            ("let a = 1;\nfoo(]);\n", (2, 5, "syntax error")),
            // Stops at the outer ERROR rather than a nested one at 1:15.
            (
                "function f(a: string {\n  return a;\n}",
                (1, 1, "syntax error"),
            ),
        ] {
            let mut parser = Parser::new();
            parser
                .set_language(&Language::TypeScript.grammar())
                .expect("grammar");
            let tree = parser.parse(source, None).expect("tree");
            assert_eq!(first_error(&tree.root_node()), expected, "{source:?}");
        }
    }

    #[test]
    fn recovered_syntax_never_delivers_a_valid_prefix() {
        let mut delivered = 0;
        let result = visit(
            "import 'valid'; function (",
            Path::new("source.ts"),
            Language::TypeScript,
            &AnalysisBudget::default(),
            false,
            |_| {
                delivered += 1;
                Ok(())
            },
        );
        assert!(
            matches!(result, Err(ExtractError::ParseError { .. })),
            "recovery is a failed parse"
        );
        assert_eq!(
            delivered, 0,
            "validate the complete tree before invoking the sink"
        );
    }

    #[test]
    fn traversal_admission_and_sink_failure_stop_delivery() {
        let source = "import 'a'; import 'b'; import 'c';";
        let admission_budget = AnalysisBudget::new(AnalysisLimits {
            max_work: 1,
            ..AnalysisLimits::default()
        })
        .expect("positive limit");
        let mut delivered = 0;
        let admission_result = visit(
            source,
            Path::new("source.ts"),
            Language::TypeScript,
            &admission_budget,
            false,
            |_| {
                delivered += 1;
                Ok(())
            },
        );
        assert!(
            admission_result.is_err(),
            "reserve traversal work before delivery"
        );
        assert_eq!(delivered, 0, "no prefix from rejected work");
        assert_eq!(
            admission_budget
                .check()
                .expect_err("sticky failure")
                .resource(),
            Resource::Work,
            "source traversal uses the work quota"
        );
        let sink_budget = AnalysisBudget::new(AnalysisLimits {
            max_imports: 1,
            ..AnalysisLimits::default()
        })
        .expect("positive limit");
        let sink_result = visit(
            source,
            Path::new("source.ts"),
            Language::TypeScript,
            &sink_budget,
            false,
            |_| {
                sink_budget.consume(Resource::Imports, 1)?;
                delivered += 1;
                Ok(())
            },
        );
        assert!(sink_result.is_err(), "sink error is a failed traversal");
        assert_eq!(delivered, 1, "stop at the first failed import admission");
    }

    #[test]
    fn grammar_tables_preserve_nested_runtime_edges_across_languages() {
        let nested = "require('a', require('b')); import('c', {with: {nested: require('d')}}); (() => require('e'))(); require(require('f')); require('ignored' + require('g')); export const nested = () => import('h'); export function run() { return require('i'); }";
        for language in [
            Language::JavaScript,
            Language::Tsx,
            Language::TypeScript,
            Language::JavaScript,
        ] {
            let jsx = match language {
                Language::JavaScript | Language::Tsx => {
                    "const component = <Pane attr='not-an-import'/>;"
                },
                Language::TypeScript => "",
            };
            let source = format!("{nested}\n{jsx}");
            let mut found = Vec::new();
            visit(
                &source,
                Path::new("nested"),
                language,
                &AnalysisBudget::default(),
                false,
                |literal| {
                    found.push(
                        crate::syntax::module_literal(literal)
                            .expect("literal value")
                            .into_owned(),
                    );
                    Ok(())
                },
            )
            .expect("valid nested program");
            assert_eq!(
                found,
                ('a'..='i')
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn type_names_and_wrappers_preserve_runtime_literal_selection() {
        let source = "type T = import('types').T; const yes = (require as Function)('yes'); (other as 'require')('ignored'); require(variable as 'ignored'); import('dyn' as string);";
        for language in [Language::TypeScript, Language::Tsx] {
            let mut found = Vec::new();
            visit(
                source,
                Path::new("typed"),
                language,
                &AnalysisBudget::default(),
                false,
                |literal| {
                    found.push(
                        crate::syntax::module_literal(literal)
                            .expect("literal value")
                            .into_owned(),
                    );
                    Ok(())
                },
            )
            .expect("valid type wrappers");
            assert_eq!(found, ["types", "yes", "dyn"]);
        }
    }

    #[test]
    fn type_only_imports_are_skipped_only_when_requested() {
        let source = "import type { A } from 'a';\n\
            import type B from 'b';\n\
            import type * as C from 'c';\n\
            export type { D } from 'd';\n\
            import type E = require('e');\n\
            type F = import('f').F;\n\
            let g: typeof import('g');\n\
            interface H extends Base<import('h').H> {}\n\
            const i = value as import('i').I;\n\
            const j = value satisfies import('j').J;\n\
            function k(x: import('k').K): void {}\n\
            class L implements import('l').L {}\n\
            import { type M } from 'm';\n\
            import n from 'n';\n\
            export * from 'o';\n\
            import P = require('p');\n\
            const q = import('q');\n\
            const r = (import('r') as Promise<unknown>);\n\
            const s = require('s') satisfies object;\n";
        let collect = |skip| {
            let mut found = Vec::new();
            visit(
                source,
                Path::new("typed.ts"),
                Language::TypeScript,
                &AnalysisBudget::default(),
                skip,
                |literal| {
                    found.push(
                        crate::syntax::module_literal(literal)
                            .expect("literal value")
                            .into_owned(),
                    );
                    Ok(())
                },
            )
            .expect("valid source");
            found
        };
        let all = collect(false);
        assert_eq!(all.len(), 19, "{all:?}");
        assert_eq!(collect(true), ["m", "n", "o", "p", "q", "r", "s"]);
    }

    #[test]
    fn consumed_text_rejects_short_buffers_and_split_utf8_boundaries() {
        let mut parser = Parser::new();
        parser
            .set_language(&Language::TypeScript.grammar())
            .expect("grammar");
        let tree = parser.parse("a", None).expect("tree");
        let node = tree.root_node();
        let path = Path::new("range.ts");
        assert_eq!(required_text(&node, "a", path).expect("valid bytes"), "a");
        for source in ["", "é"] {
            let error = required_text(&node, source, path).expect_err("invalid foreign byte range");
            assert!(matches!(error, ExtractError::ParseError { .. }));
            assert!(error.to_string().contains("invalid source range"));
        }
    }
}
