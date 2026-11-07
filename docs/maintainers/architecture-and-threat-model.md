# Architecture and threat model

Owner: alm. Reviewed against Rust-OE STD-014/019/021/033/065/066,
INPUT and CLI at `19e4133c0461129b0f5fdb286abd7b87511242e2`.
Revisit on a parser/dependency update, new input/output/API, concurrent filesystem
support, remote repository exposure, new subprocess, or release-privilege change.

The production path is arguments/config/stdin → package/workspace discovery →
shared metadata snapshots → parallel regular-file readers → native syntax trees →
owned import records → indexed file/package graphs → iterative search → owned
results → bounded staged report → stdout. Errors flow back to the single output
owner. Metadata/relative-resolution caches do not execute JavaScript. The CLI
owns no persistent database, credential store, network listener, or project writer.

The Rust library exposes owned graph/results and borrowed import slices. Dense
analysis IDs are interned before use, adjacency is immutable during traversal,
and traversal state is initialized before frame access. SCC/DFS/Johnson/unblock
use heap stacks. This replaces call-stack depth with finite graph/work/cycle
admissions; it does not provide OS process isolation. Shared counters are atomic,
monotonic and sticky. Cache mutex guards do not span file I/O or parsing; poisoned
cache guards are recovered because entries are independent optional memoized
lookups, with no unsafe invariant depending on partial mutation.

Tsconfig raw-document sharing uses a per-file OnceLock after releasing the map
mutex. An initializer reads, parses and admits one document only; it never loads
an extends parent while initializing that cell. Thus concurrent children share
physical reads without introducing recursive waits on cyclic extends. Depth is
checked on every inheritance occurrence, and child origin/field selection is
recomputed from immutable raw values. Raw retained payloads and compiled/owned
copies have separate cumulative admissions. Absolute selected keys distinguish
caller working directories; physical resolution of parent-containing discovery
operands prevents a syntactic upward walk from descending into an unrelated
caller directory. Successful snapshots outlive source removal by design; stable
checkout input remains the operational requirement.
Rebinding a cache to a budget starts empty caches, preventing cross-budget reuse
of prior raw documents, compiled snapshots or directory results. Ordinary clones
retain their shared cache and original budget.

File-alias results retain canonical identities in a separate specifier namespace.
Their payload is admitted before insertion, including a recheck after concurrent
misses; missing matches are distinguished from external names. Every extraction
clears these entries before workers start. Lexer-name decoding and transparent
TypeScript expression traversal allocate no decoded callee string and remain
bounded by the admitted source size.
Alias matching work is admitted on cache misses rather than repeated for hits
or unrelated relative/workspace imports. Relative and alias candidates share
lexical dot-segment normalization before filesystem lookup, following the selected
JS/TS module policy; an existing target's canonical path supplies graph identity.
Workspace alias setup validates a sufficient ordering/uniqueness invariant once
before direct exact-mapping resolution. Public caller-constructed tables retain
general first-match behavior when that invariant does not hold. Workspace name
classification and path lookup both prioritize declared package names over a
conflicting alias; a quota failure preserves the previous published alias map.
Within one eligible setup, equal ordered replacement lists share normalized
targets. Borrowed keys stay within the config's lifetime; owned target values and
cache records are admitted before retention. General caller-created tables retain
name-dependent first-match lookup. The cache is local to setup, so repeated setup
observes new fallback files without reusing prior results.

Extraction retains a bounded set of positive regular `index.ts` paths from its
canonical-root walk, without following child directory links. The set admits a
record and path bytes before retention, stays separate from file/directory/alias
cache namespaces, and resets on every extraction. File candidates still precede
index lookup; unknown or differently spelled paths use the filesystem. The walk
is not evidence of absence. Workspace-only mode skips hint retention. Sources
retain their existing canonicalization and pre-open/opened-handle checks; this
does not strengthen the stable-checkout assumption into race-proof isolation.

The native parser boundary uses the willbooster-tree-sitter safe Rust bindings and
the JavaScript and TypeScript/TSX grammar packages. Grammar handles come only from the pinned grammar crates' static language
constants. The fork runtime and grammars share a private ABI number; the runtime
rejects a grammar with any other ABI, which `visit` reports as a parse error.
Arbitrary foreign pointers, callbacks or allocator transfers are not accepted by
this product. Provider safe wrappers own
trees and borrow node text/lifetimes. jscycles introduces no unsafe operation or
FFI declaration; that restriction does not establish dependency soundness. The
[build trust inventory](build-trust.md) identifies executable build-time inputs.
The private language adapter replaces the general language package so unrelated
native grammars are not compiled just to select JS/TS. Existing native fixtures
exercise the supported adapter; the source_syntax fuzz target selects these same
three grammar handles under Rust/native address sanitization. The semantic
harness now runs the production cursor visitor as well. Source length must fit
the provider's u32 byte range even when configured quotas are raised. Cursor
iteration avoids repeated ancestry validation and introduces no recursive Rust walk.

The visitor derives immutable numeric-kind classifications from each linked
grammar's metadata, rather than requesting a kind-name string at every node.
Each symbol ID is classified, including aliased IDs with the same name, and
lookup is bounds checked. The three caches own only Rust enums; native parsers
and trees remain local to a file. No native handle/source lifetime is extended.
All nodes retain source-range validation, and actual literal/identifier text reads
share a fallible byte/UTF-8 check. Nested calls and grammar switches have explicit
native regressions and a fresh sanitizer semantic campaign.

| Threat / boundary | Mitigation and observable control | Residual risk / disposition |
| --- | --- | --- |
| Hostile file counts, bytes, depth and dense graphs exhaust resources | Shared admission quotas before queues/retention, bounded readers/YAML, iterative graph traversal, work/cycle/payload limits; exact and first-excess tests | Native parsers/OS calls are not preempted; malloc/capacity overhead is not exact RSS. alm limits use to local checkouts; remote multi-tenant scanning requires OS isolation first. |
| Error recovery or swallowed metadata/config errors invent a clean scan | Strict selected-input/config ingress, syntax-error rejection, all-worker completion, sticky budget errors, staged output | Automatic ordinary metadata fallback and static graph limitations are explicit compatibility decisions in the product contract. |
| Escapes, alias inheritance or path spelling hide/invent graph edges | Decode string values, config-origin tracking, child-option replacement, fallback/index resolution and canonical node identity; real process regressions | Full runtime/module/compiler resolution is outside the named static scope. |
| Untrusted filenames/manifests control a terminal or machine report | Human labels escape controls/bidi controls; JSON serialization escapes string controls; source/package paths reject invalid UTF-8 | Source/path names remain private data; redirects and CI logs are owned by the operator. No secret-bearing configuration is supported. |
| Symlink/path replacement reads outside intended data or changes a scan | Recursive walks do not follow child-directory symlinks; explicitly selected roots may be canonicalized; regular-file checks before/after open; no writes/project execution | Caller-selected outside paths and file symlinks are allowed. No descriptor-based confinement or snapshot consistency against hostile concurrent mutation is claimed. |
| Cached misses or copied snapshots become stale/unbounded | Extraction-scoped resolution cache reset, Arc metadata snapshots, one-scan budget/accounting and explicit cache lifetime | Caller must create a new metadata cache after changes and keep checkout stable during a scan. |
| Native parser memory/ABI bug | Safe typed provider adapter, selected grammar versions, compiler/target tests, bounded deterministic syntax fuzzing with address sanitization | Passing tests/sanitizers do not prove native soundness; maintain advisory/update response and isolate remote inputs. |
| Build scripts/macros/actions execute compromised code | Locked reviewed sources, checksums, explicit deny policy, narrowed grammar/dependency surface, pinned actions and least-privilege job tokens | Not a full third-party code audit, reproducible build or signed provenance assessment. No blanket Vet exemptions are created. |
| Failed/mixed release artifacts are published or replace an existing version | Matching-tag requirement, exact inventory, source/run/lock consistency, digest validation and native binary smoke tests before release action | Host tag/review permissions and post-publication verification remain external controls; failures after publication require the release owner runbook. |
| Reports/source are sent to a third party | No runtime networking/upload/project scripts; development artifacts stay local or in explicitly configured CI retention | No external duplicate triage, telemetry or code-review service is authorized by installing a tool. |
| Sole maintainer unavailable / vulnerability not privately reportable | [Lifecycle runbook](maintainer-runbook.md) identifies needed procedures and unresolved owner/access facts | Private reporting route and alternate privileged recovery owner are OPEN; documentation cannot fabricate their operational existence. |

Alternatives: recursive traversal was rejected for deep checkout graphs; unlimited
elementary enumeration was rejected for exponential output; direct stdout was
replaced by staging so admission failures publish nothing; per-worker budgets
were rejected because aggregate limits would multiply with concurrency. Byte
quotas reserve known payloads while count/work quotas constrain container growth.
A server process supervisor, capability filesystem and hard RSS/deadline limits
would be required for a different remote execution boundary.
