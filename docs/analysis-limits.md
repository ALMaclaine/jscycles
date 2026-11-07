# Analysis limits

One CLI invocation shares an `AnalysisBudget` across discovery, workspace/tsconfig
metadata, source queues and readers, parallel package/file workers, file and
package graph construction, cycle search, evidence expansion, and report staging.
A failed admission is sticky: optional metadata fallback cannot clear it. A quota
failure exits 2 with a diagnostic and empty stdout, including in quiet/JSON modes.
A complete report is staged before the CLI writes it to stdout; a subsequent OS
write/flush failure can still leave partial stdout and exits 2.

Set positive integer quotas under `limits` in `jscycles.yaml`. Unspecified fields
use these defaults; unknown fields and zero quotas are errors.

| Setting | Default | Unit / scope |
| --- | ---: | --- |
| `max_entries` | 1000000 | Visited filesystem entries, including excluded entries; pruned descendants are not visited |
| `max_files` | 100000 | File admissions for metadata reads and queued sources; repeated reads/scans count again |
| `max_total_bytes` | 536870912 | Bytes read from all admitted source and metadata files |
| `max_file_bytes` | 16777216 | Maximum bytes in one source/metadata file, checked by metadata and during reading |
| `max_imports` | 2000000 | Admitted import records |
| `max_nodes` | 500000 | Nodes admitted across all file and package analysis graphs |
| `max_edges` | 2000000 | Distinct edges admitted within each graph, summed across graphs |
| `max_cycles` | 10000 | File representatives and elementary package cycles combined |
| `max_work` | 20000000 | Source bytes admitted before parsing, plus logical traversal, matching, sorting, backtracking, and expansion operations |
| `max_graph_bytes` | 134217728 | Cumulative retained input-path, import, metadata snapshot/cache, graph-label, and evidence reservations |
| `max_cycle_bytes` | 33554432 | Cycle path indices/labels and expanded evidence reservations |
| `max_output_bytes` | 33554432 | Encoded report bytes, including JSON whitespace and the final newline |

For example, to permit more entries and files while lowering the cycle quota:

```yaml
limits:
  max_entries: 2000000
  max_files: 200000
  max_cycles: 2000
```

Reservations are cumulative and are not refunded. File counts are admissions,
not unique paths: workspace discovery and package discovery can read the same
manifest, and overlapping selected packages can queue a source twice. Counters
for nodes/edges likewise include separate analysis graphs. `max_file_bytes`
tracks the largest proposed single-file size instead of summing file sizes.
Conservative reservations can reject a scan before its actual retained payload
reaches the byte quota. Raise a relevant quota deliberately for a trusted larger
checkout; no setting disables quotas implicitly.

The configuration that selects these limits must first be read and parsed. That
bootstrap has a fixed 16 MiB cap and the YAML parser's built-in alias/depth
budgets; it is excluded from the shared byte/file counters. Stdin separately
retains its fixed 1 MiB / 10000-path limits. Tsconfig extends retains its depth-ten
limit. Automatic tsconfig and optional workspace aliases still tolerate ordinary
missing/malformed metadata, but an exhausted shared budget always fails the scan.
Explicit `--tsconfig` remains strict.

Graph SCC discovery, file-cycle search, package-cycle search, and Johnson
unblocking all use explicit heap stacks. File analysis returns one deterministic
representative per cyclic SCC, including self-loops. Package analysis enumerates
all elementary cycles between distinct packages until a quota fails; it never
returns a truncated successful result. `--outer` skips file-graph construction
and file-cycle enumeration, including when no workspace exists.

These quotas bound admissions and selected work, not exact RSS or elapsed time.
Container overhead and allocator capacity are controlled by the count quotas;
byte quotas reserve documented payloads rather than measuring the allocator.
Native parsing, filesystem calls, string/glob matching, and sorting comparisons
can perform multiple machine operations per logical unit. Concurrent native
parsers can finish an admitted file before observing another worker's failure.
There is no hard wall-clock cancellation or process memory limit. Hostile remote
repository scanning still needs process isolation and OS resource limits.

## Known analysis limits

- `require` is recognized by name, not by scope. A local parameter or variable
  named `require` that is called with a literal still creates an edge, which can
  report a cycle Node would not load. Rename the binding or `ignore` the file.
- Rust's runtime reopens a closed standard stream on `/dev/null` at startup, so
  running with stdout closed (`>&-`) is indistinguishable from `> /dev/null`: the
  report is discarded and the exit status still reflects the scan. Write and
  flush failures on a real stream (for example a closed pipe) exit 2.
- Directory symlinks beneath a walk root are not followed (R08). A workspace
  member that is a directory symlink is therefore not a member, and its imports
  are external; a linked directory outside a package is not walked, so an edge
  into it has no outgoing edges. Either can hide a cycle. Use real directories
  or pass the link target as an operand. File symlinks are followed.
- On Windows, JSON cycle paths use native `\` separators and paths outside a
  package root may print as verbatim `\\?\` paths.
- A cycle formed only by relative imports that cross package directories (for
  example `packages/a/x.ts` importing `../b/y` and back) is not reported: each
  package analyzes only its own files, and relative paths do not create package
  edges (R02). Import other packages by name.
- A `package.json` operand selects that package alone without discovering
  packages nested in it, so it analyzes their files too. Pass the directory to
  get nearest-package ownership.
- Each package uses one tsconfig: the nearest at or above the package
  directory. A `tsconfig.json` nested inside a package (for example `src/`) is
  not consulted, whereas an editor may apply it to files beneath it.
- Syntax rejection depends on the pinned grammars. If valid code fails to parse,
  report it with the printed line and column; `ignore` the file meanwhile.
- YAML quota values are deserialized from scalars, so a quoted number such as
  `"5"` is accepted as `5`. Explicit `null` package overrides are rejected.

## Library use and compatibility

`AnalysisLimits`, `AnalysisBudget`, `Resource`, `LimitError`, and `BoundedReport`
are public in `jscycles::budget`. Clone a budget handle to share atomic counters;
create a new budget for each new operation. `check()` exposes the first recorded
exhaustion. Do not accept a partial result or reuse an exhausted budget.

Discovery, workspace/tsconfig loading, and extraction expose budget-aware
constructors/loaders; the CLI uses one handle throughout. Tsconfig/workspace
snapshots can be shared through `Arc`. Legacy optional metadata APIs retain
best-effort return types; callers using those with a shared budget must call
`check()` before accepting success. Legacy `Config::load_or_default` and the
infallible workspace-package selection convenience API retain their fallback
and allocation behavior; they are not the CLI's bounded entry points.

`TsConfigCache::with_budget` starts empty caches, including when called on a used
cache or its clone. Values admitted by a previous budget cannot satisfy a new
operation's reads or payload reservations. Ordinary clones continue sharing
snapshots and their original budget.

Owned `TsConfigCache::discover/load` copies reserve their known string/path/record
payloads and work before cloning; `discover_shared` retains shared snapshots.
A failed copy returns None with sticky exhaustion. Alias-policy and package-override
sorting are admitted under logical work before sorting; bounded package selection
reuses its selected override instead of sorting it twice.

TsConfigCache additionally shares raw parsed documents by canonical file identity.
Each successful physical read consumes file/byte admissions once per cache;
concurrent children wait for that document's initializer. Every extends occurrence
still consumes logical work and checks the ten-document depth limit. Only fields
still needed by the child are copied, but every parent is read and type-checked.
Cache keys, initialization cells and known raw string/path/map-record payloads
consume retained-byte admissions before retention, including overridden parent
values. Compiled mappings and owned getter copies retain their own admissions.
Failed initialization may retry the typed reader without retaining another raw
snapshot, and existing exhaustion always rejects cache hits. No quotas are raised.

Per-extraction file-alias memoization reserves the specifier and canonical target
payload before retaining an entry. Missing matched targets are memoized too;
unmatched external names are not. A second lookup after filesystem work prevents
duplicate cache reservations when workers race. Cache hits and misses are cleared
before the next extraction, and alias admission failures propagate as limit errors.
Alias-table pattern/replacement work is admitted only on an actual uncached
alias lookup, before traversing the table. Relative imports, known workspace
packages, and cached matched targets do not traverse or reserve that table again.
Source-byte/import/payload admissions still apply to every record. Distinct
uncached searches remain bounded, and a prior exhaustion still rejects hits.
Workspace alias setup checks the compiled table's ordering/uniqueness and
nonempty replacements once. Eligible exact mappings resolve directly, with work
charged for the selected mapping instead of a whole-table search per alias.
Caller-constructed tables with other ordering, duplicates or empty replacements
retain general first-match lookup and its full search admission. Package-membership
matching and retained alias strings remain admitted. A failed setup publishes no
partial replacement of the workspace's existing alias map.

Eligible exact workspace aliases also share normalized target lookups for equal
ordered replacement lists during one alias setup. Cache keys borrow the selected
config; owned target paths and known cache records consume retained-byte admission
before insertion. Key hashing consumes logical work on hits and misses, and
package-membership checks and published alias strings retain their admissions.
Distinct target lists remain bounded by the payload quota. General caller-built
tables keep per-name first-match resolution. The cache is dropped after each
setup, including failures; no new cache survives a metadata reload.

Full extraction also retains positive `index.ts` paths discovered as regular
files beneath the canonical walk root. Each hint reserves its PathBuf record and
native path bytes before insertion; lookup work is admitted before probing the
hint set. Only the first index candidate is eligible. An unselected file, later
index extension, differing spelling or file link still uses filesystem fallback;
the selected file list establishes no negative lookup result. Hints reset for
every extraction. Workspace-only extraction constructs no index hints. Source
canonicalization and pre-open/opened-handle regular-file checks are unchanged.

Grammar classification tables are fixed binary metadata, initialized once for
each of the three linked grammars. Their size is bounded by those grammars' u16
symbol counts; source text cannot grow them. They retain only Rust enum values,
not sources, trees, parser state or checkout metadata. They are separate from the
operation's cumulative input/cache admissions. Source-dependent work, native u32
length checks, whole-tree rejection and all source-range checks remain unchanged.

`CycleDetector::try_detect_imports` and the package detector's
`try_detect_imports` / `try_detect_imports_with_files` borrow imports and admit
graph nodes/edges before allocating their analysis graph. `try_detect` checks
analysis of a caller-owned graph; the caller already owns and has allocated
that input. The existing `detect` and `detect_with_files` signatures remain
infallible and have no quotas; they use the same iterative algorithms. Choose
the fallible APIs when bounding search or enumeration is required.

Adding `Config::limits` and quota variants to public error enums changes source
compatibility for struct literals and exhaustive matches. Together with the
previous YAML error and canonical-path changes, this belongs in the next 0.2
library release. No version bump or publication is performed here.

## Evidence and adopted rules

This implements INPUT-003 (aggregate boundary budgets), INPUT-004/005 (domain
invariants and checked accounting), INPUT-009 and CLI-003/004/006/008 (failure,
output, and worker completion), and API-008/009/010 (tests, shared mutation, and
compatibility) from Rust-OE `19e4133c0461129b0f5fdb286abd7b87511242e2`.
The shared counters use checked atomic admission; no global or thread-local
budget is installed. Narrow private indexing exceptions rely on interned dense
IDs, immutable adjacency, initialized traversal state, and checked frame cursors.

`tests/budgets.rs` tests all twelve CLI quota failures in JSON and quiet modes,
exact encoded output size, overflow, shared concurrent counters, multiple package
workers, optional metadata exhaustion, excluded entries, evidence expansion,
dense-graph cycle/work cutoffs, and shared metadata snapshots. It compares all
4096 directed graphs on four vertices without self-edges against an independent
simple-path enumerator and checks file SCC representative counts by reachability.
Existing file self-loop tests remain. Both graph types also traverse a 20000-node
chain/ring in a thread with a 128 KiB stack. The bounded input reader tests exact
per-file and cumulative byte limits. These are ordinary regression evidence,
not a fuzzing campaign or a proof for every graph.
