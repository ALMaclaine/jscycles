//! Literal import forms must produce the same observable graph.
use std::{fs, process::Command};

use jscycles::ImportExtractor;

#[test]
fn literal_call_forms_preserve_real_cycles() {
    for (extension, source) in [
        ("js", "void import('./b', {with: {type: 'json'}});"),
        (
            "js",
            "void import(/* before */ ('./b'), {with: {type: 'json'}});",
        ),
        ("js", "require('./b', true);"),
        ("js", "require?.('./b');"),
        ("js", "(require)('./b');"),
        ("ts", "require<unknown>('./b');"),
        ("ts", "import B = require('./b');"),
        ("ts", "type B = import('./b').B;"),
        ("tsx", "void import('./b', {with: {type: 'json'}});"),
        ("js", r"\u0072equire('./b');"),
        ("js", r"requ\u0069re('./b');"),
        ("js", r"\u{72}equire('./b');"),
        ("ts", "require!('./b');"),
        ("ts", "(require as any)('./b');"),
        ("ts", "(require satisfies Function)('./b');"),
        ("ts", "(<any>require)('./b');"),
        ("ts", "(require<unknown>)('./b');"),
        ("tsx", "(require<unknown>)('./b');"),
        ("ts", "require('./b' as const);"),
        ("ts", "require('./b' satisfies string);"),
        ("ts", "require('./b'!);"),
        ("ts", "require(<string>'./b');"),
        ("ts", "void import('./b' as string);"),
        ("tsx", "(require as any)('./b' as const);"),
    ] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fs::write(root.join("package.json"), r#"{"name":"literal-forms"}"#).expect("manifest");
        fs::write(root.join(format!("a.{extension}")), source).expect("source");
        fs::write(root.join(format!("b.{extension}")), "import './a';").expect("cycle edge");
        let result = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(root)
            .args([
                "--inner",
                "--no-tsconfig",
                "--json",
                "--extensions",
                extension,
            ])
            .output()
            .expect("CLI starts");
        assert_eq!(
            result.status.code(),
            Some(1),
            "{extension}: {source}; stderr={}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("JSON");
        assert_eq!(report["has_file_cycles"], true, "literal edge retained");
    }
}

#[test]
fn member_calls_and_computed_values_do_not_invent_import_edges() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::write(
        directory.path().join("source.ts"),
        r#"
        object.require('./member');
        require.resolve('./resolved');
        require('./prefix' + suffix);
        require(...['./spread']);
        import(`./${dynamic}`);
        (object.require as Function)('./typed-member');
        (require.resolve as Function)('./typed-resolver');
        require((prefix + './computed') as string);
        require(variable as './type-literal');
        require(<'./type-literal'>variable);
        (other as 'require')('./wrong-callee');
        (<'require'>other)('./wrong-asserted-callee');
        require((prefix, './sequence') as string);
        require('./conditional' ? yes : no);
        requir\u0066('./different-name');
        const text = "require('./quoted')";
        // import('./commented');
    "#,
    )
    .expect("source");
    let imports = ImportExtractor::new(vec!["ts".to_owned()], None)
        .extract(directory.path())
        .expect("valid source");
    assert!(imports.is_empty(), "only bare literal calls form edges");
}

#[test]
fn deeply_nested_type_assertions_remain_transparent_without_recursive_rust_walks() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let source = format!(
        "require({}'./target'{});",
        "(".repeat(512),
        " as string)".repeat(512)
    );
    fs::write(directory.path().join("source.ts"), source).expect("source");
    let imports = ImportExtractor::new(vec!["ts".to_owned()], None)
        .extract(directory.path())
        .expect("valid nested type assertions");
    assert_eq!(
        imports.len(),
        1,
        "retain the literal through every assertion"
    );
    assert_eq!(
        imports[0].specifier, "./target",
        "preserve the actual value"
    );
}

#[test]
fn deeply_parenthesized_literals_are_walked_iteratively() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let source = format!(
        "require({}'./target'{});",
        "(".repeat(2048),
        ")".repeat(2048)
    );
    fs::write(directory.path().join("source.js"), source).expect("source");
    let imports = ImportExtractor::new(vec!["js".to_owned()], None)
        .extract(directory.path())
        .expect("valid nested literal");
    assert_eq!(
        imports.len(),
        1,
        "one source edge despite transparent parentheses"
    );
    assert_eq!(imports[0].specifier, "./target", "literal value preserved");
}
