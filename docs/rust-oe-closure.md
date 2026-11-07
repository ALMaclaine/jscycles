# Rust-OE rule evidence and closure review

The subsequent [engineering tool pass](quality-engineering.md) supersedes this
record's Clippy/CRAP/mutation/Miri/Hotpath deferrals. Earlier numeric validation
reports retain their recorded source scope.
The [combined-tree review](rust-oe-current-review.md) refreshes the ledger's
superseded tool/feature/performance decisions and implementation references.
Its validation is separate from the historical counts below.

Owner: alm. Source revision: `19e4133c0461129b0f5fdb286abd7b87511242e2`.
Reviewed 2026-11-04. The review began with configuration/workspace failures and
continued through public APIs, graph invariants, native parsing, tools/supply
chain, performance/storage and lifecycle requirements after the broader
[applicability audit](rust-oe-full-audit.md). Full adherence remains unestablished.

The [rule ledger](rust-oe-closure-ledger.json) records a disposition for every
claim in the complete 521-record [source inventory](rust-oe-rule-inventory.json).
Each entry retains the exact source ID/hash, selected contract, implementation
symbols, tests/checks and limits. Entries distinguish evidence review, context,
scope decisions, exceptions and conditional release/tool work. A row count is
neither a count of satisfied requirements nor a compliance percentage. The
initial 58 detailed boundary records have been extended with individual
implementation/evidence, context, profile, conditional and operational decisions
for the remainder. A source catalog or inactive architecture is not a product gate.

## Counterexamples and repairs

The first regression run had eight assertion failures and two passing boundary
tests. A later run exposed two more behavior failures plus a filesystem setup
failure. The setup failure is deliberately excluded from product-defect counts.

| Boundary | Observed defect | Repair / consumer regression in tests/boundary_closure.rs |
| --- | --- | --- |
| Workspace metadata | A dangling pnpm/package/tsconfig entry could be treated as absent | Fallible optional metadata distinguishes absent entries from broken links/errors; `dangling_workspace_metadata_is_an_error_instead_of_absence` |
| Workspace pattern root | An existing regular file in the directory prefix produced zero packages | Require a directory or return its contextual error; `workspace_pattern_prefix_must_be_a_directory` |
| Project references | Missing reference roots were silently discarded | Propagate the failed reference read; `missing_typescript_reference_is_an_error` |
| Project references | A reference to a named config file lost package membership and could return clean | Use the referenced file's directory for package membership; `typescript_references_accept_named_config_files` |
| Package identity | Duplicate names were accepted in inner scans without workspace metadata | Validate canonical identities in fallible discovery; `duplicate_names_fail_without_workspace_metadata_in_every_output_mode` |
| Selection overlap | A directory, its manifest and an ancestor processed one package three times | Deduplicate the same name/canonical-directory identity before workers; `overlapping_operands_select_a_package_once` |
| Extends resolution | `base.profile` could load `base.json` and lose a real edge | Append `.json` to the complete stem; `extends_appends_json_without_replacing_a_dotted_stem` |
| Error context | Malformed inherited config named only the requested child | `TsConfigError::AtPath` retains the failing file and typed cause; `inherited_config_failures_identify_the_actual_failing_file` |
| Workspace globs | A single `*` admitted packages several directories deep | Require literal separators for include/exclude patterns; `workspace_star_matches_one_directory_component` |
| Alias substitution | A literal checkout asterisk became substitution syntax | Only wildcard aliases substitute their configured marker; `checkout_asterisks_are_literal_in_exact_and_wildcard_alias_targets` |

Further API/parser/resource review reproduced and repaired these cases:

| Boundary | Observed defect | Repair / regression |
| --- | --- | --- |
| Module values | Valid braced Unicode escapes with leading zeros or paired surrogate units were rejected | Bound the code-point value, preserve UTF-16 units until final validation; `escaped_import_specifiers_use_their_javascript_value` and `braced_surrogate_escapes_can_form_a_valid_unicode_module_value` in tests/adherence.rs |
| Workspace parser errors | Converting JSON/YAML errors to strings lost their typed causes | Preserve JSON causes and a boxed YAML path/cause context; `workspace_parser_errors_retain_their_typed_causes` |
| File resolution | Missing alias/relative targets could become resolved graph vertices | Keep those imports Unresolved; `missing_alias_and_relative_targets_do_not_invent_resolved_graph_nodes` |
| Cache retention | Repeated owned cache getters cloned metadata outside the shared payload counter | Admit complete copies before cloning; `owned_cached_metadata_copies_observe_the_shared_payload_budget` |
| Policy sorting | Alias and package-override sorts bypassed logical work admission | Admit sorts and avoid repeating override selection; `tsconfig_alias_sorting_requires_work_admission` and `package_override_sorting_requires_work_admission` |

The literal changes follow the
[ECMAScript StringValue/UTF-16 contract](https://tc39.es/ecma262/2026/#sec-literals-string-literals).
The recorded pre-repair release binary rejected both valid forms with status 2;
the current process tests require the actual cycle and status 1. Legacy numeric
escapes remain explicitly unsupported. Missing declared alias candidates remain
available to workspace membership, while file classification requires a file.

The project-reference file form is part of the
[TypeScript reference contract](https://www.typescriptlang.org/docs/handbook/project-references).
This scanner uses references for package membership; it does not build/type-check
referenced projects or promise the full compiler's module-resolution behavior.
An existing reference directory without a named package remains a nonpackage.
Unmatched workspace patterns remain valid empty selections. An existing invalid
pattern prefix is an input failure.

Native config origins also used a lossy String conversion. They now stay in
PathBuf, and wildcard substitution preserves native ancestor components. The
Unix API regression `alias_results_preserve_native_origin_bytes` executes on
macOS. The end-to-end `config_origin_paths_do_not_undergo_lossy_unicode_conversion`
test is Linux-only: this Mac filesystem rejects its non-UTF-8 directory name
with `Illegal byte sequence` before the program runs. Its actual Linux execution
remains G03; it is not reported as a repaired macOS process counterexample.
Source/package paths printed in the current report schema must still be UTF-8.

Two additional tests establish existing behavior: duplicate YAML map keys reject,
and ten files in an extends chain succeed while an eleventh fails. Internal
compiler-option tests now use the native path representation; their inheritance
assertions are preserved. No snapshot or existing expected result was relaxed.

## Verification and limits

[Historical validation evidence](rust-oe-closure-validation.json) records the source hashes,
commands, actual exits/counts, pre-repair failures and classification of the setup
failure. Native macOS aarch64 runs 431 target tests and one doctest on development
Rust 1.98.1 and exact MSRV 1.92.0. Other hosts have conditional test selections.

`python3 scripts/verify-controls.py --boundaries --output
docs/rust-oe-closure-controls.json` runs the original 20 controls and nine
additional pass/fault/repair cases on Unix. The three named changes reverse the
duplicate-directory comparison, relax component glob matching and introduce a
lossy alias-result conversion. Each changes only a disposable copy; each must
make its selected consumer test fail by assertion and pass after restoration.
The [29-case report](rust-oe-closure-controls.json) verifies the original source
remained unchanged. These are bounded fault controls, not broad mutation
coverage, a survivor score or a new required CI tool.

The [coverage review](rust-oe-closure-coverage.json) records the current Rust line
scope and its limits. Updated short sanitizer campaigns exercise
[configuration](rust-oe-closure-fuzz-configuration.json),
[native syntax](rust-oe-closure-fuzz-syntax.json) and the
[actual private text adapters](rust-oe-closure-fuzz-text.json). The new text target
checks independent JSON/UTF-16 values and JSONC byte offsets/strict-JSON semantic
preservation, with a separately bounded generated literal. No target fuzzes
filesystem membership/resolution. Prior reports retain their original input identity.

Automatic ordinary tsconfig alias failure and legacy best-effort/unmetered APIs
remain named compatibility exceptions. Workspace membership failures and shared
quota exhaustion propagate. Native parser trust, hostile filesystem mutation,
hard OS isolation and full compiler conformance remain separate boundaries.

## Remaining closure work

Every source claim now has a scoped review disposition; this does not prove
every implementation or input correct. G01–G03 still need operational security/support,
access/recovery and actual supported-host enforcement evidence. C01 remains
conditional preparation/verification at the next release boundary.

Revalidate these decisions on compiler/MSRV, parser/provider/tool, platform,
exposure, schema, configuration precedence, identity/cache, resource or failure
contract changes. This ledger adds no semantic keyword gate or organizational
certification claim.
