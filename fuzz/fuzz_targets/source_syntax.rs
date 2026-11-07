#![no_main]
use jscycles::{ExtractError, budget};
use libfuzzer_sys::fuzz_target;

#[path = "../../src/module_syntax.rs"]
mod module_syntax;
#[path = "../../src/syntax.rs"]
#[expect(
    dead_code,
    reason = "each target exercises only part of the shared private adapter module"
)]
mod syntax;

fn extracted(source: &str, extension: &str) -> Result<Vec<String>, ExtractError> {
    let path = std::path::PathBuf::from(format!("source.{extension}"));
    let budget = budget::AnalysisBudget::default();
    let mut found = Vec::new();
    module_syntax::visit(
        source,
        &path,
        module_syntax::Language::for_path(&path),
        &budget,
        |literal| {
            budget.consume(budget::Resource::Imports, 1)?;
            budget.consume(
                budget::Resource::GraphBytes,
                u64::try_from(literal.len()).expect("bounded literal"),
            )?;
            found.push(literal.to_owned());
            Ok(())
        },
    )?;
    Ok(found)
}

fn semantic_forms(text: &str) {
    let value: String = text
        .chars()
        .take(256)
        .filter(|character| *character != '\0')
        .collect();
    let literal = serde_json::to_string(&value).expect("JSON literal");
    for extension in ["js", "ts", "tsx"] {
        let mut program = format!(
            "import {{x}} from {literal}; export * from {literal};\nlet a = import(/* first */ ({literal}), {{with: {{type:'json'}}}});\nlet b = (require)(/* first */ ({literal}), true); let c = require?.({literal});"
        );
        program.push_str(&format!(
            "\n\\u0072equire({literal}); requ\\u{{69}}re({literal});"
        ));
        let mut count = 7;
        if extension != "js" {
            program.push_str(&format!("\nimport Eq = require({literal}); let d = require<unknown>({literal}); type T = import({literal}).T;"));
            program.push_str(&format!("\n(require as Function)({literal}); (require satisfies Function)({literal}); require!({literal}); (require<unknown>)({literal}); require({literal} as const); require({literal} satisfies string); require({literal}!); import({literal} as string);"));
            count = 18;
            if extension == "ts" {
                program.push_str(&format!(
                    "\n(<Function>require)({literal}); require(<string>{literal});"
                ));
                count = 20;
            }
        }
        assert!(program.len() <= 32_768);
        let found = extracted(&program, extension).expect("valid literal import forms");
        assert_eq!(found.len(), count, "every literal form contributes once");
        for raw in found {
            assert_eq!(
                syntax::module_literal(&raw).expect("JSON StringValue"),
                value
            );
        }
        // Separate bounded program: preserve the original 256-scalar and
        // worst-expansion coverage while exercising nested runtime positions.
        let nested = format!(
            "require({literal}, require({literal}));\nimport({literal}, {{with: {{nested: require({literal})}}}});\n(() => require({literal}))();\nrequire(require({literal}));\nrequire({literal} + require({literal}));\nexport const nested = () => import({literal});\nexport function run() {{ return require({literal}); }}"
        );
        assert!(nested.len() <= 32_768);
        let found = extracted(&nested, extension).expect("valid nested literal forms");
        assert_eq!(
            found.len(),
            9,
            "nested calls contribute their own literal edges"
        );
        for raw in found {
            assert_eq!(
                syntax::module_literal(&raw).expect("JSON StringValue"),
                value
            );
        }
        let mut ignored = format!(
            "object.require({literal}); require.resolve({literal}); require({literal} + suffix); require(...[{literal}]); import(dynamic); const text={literal};"
        );
        if extension != "js" {
            ignored.push_str(&format!("\n(object.require as Function)({literal}); require(variable as {literal}); (other as 'require')({literal}); require((prefix + {literal}) as string);"));
        }
        assert!(
            extracted(&ignored, extension)
                .expect("valid computed/member expressions")
                .is_empty()
        );
    }
}

// Exercise the actual safe Rust/native grammar adapter, with ASan enabled for
// Rust and native C, then the actual production syntax visitor. No filesystem,
// network or project execution is performed by either path.
fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > 32_768 {
        return;
    }
    if let Ok(source) = std::str::from_utf8(bytes) {
        semantic_forms(source);
        // Arbitrary valid/recovered inputs reach the same visitor as the CLI.
        for extension in ["js", "ts", "tsx"] {
            let _outcome = extracted(source, extension);
        }
    }
});
