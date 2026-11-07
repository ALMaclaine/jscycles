# Rust API semantic boundaries

## Applicability and authority

- **Scope:** Public or reused Rust API, ownership, conversion, trait, and representation changes.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: library/API maintainer; consulted: owners of affected downstream consumers. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Design public Rust APIs around the ownership, borrowing, conversion, and trait semantics callers observe. State the relationship between inputs and returned borrows, accept borrowed forms when ownership is unnecessary, and implement standard traits only when their semantic contracts hold. Do not expose a `?Sized`, `Deref`, conversion, or blanket-implementation surface merely to make an example or generic abstraction compile.

## Risk

Overbroad generic or blanket implementations can overlap with present or future reference cases. Incorrect standard-trait implementations produce silently wrong collection behavior, lossy conversions, surprising implicit dereferencing, or APIs whose lifecycle promises callers cannot satisfy. Treating `T: 'static` as a requirement for an object to live for the process encourages needless leaks and incorrect constraints.

## Design and review rules

1. Before adding `T`, identify whether `T` itself can be a borrowed type. Check coherence/overlap before adding a reference-specialized implementation beside a blanket implementation.
2. Prefer `&str`, slices, or another borrow for read-only access and `&mut` for in-place mutation. Owned inputs can serve retention, transfer, construction, or a justified ergonomic contract; mutation alone does not require ownership. Relate returned borrows to their source without unnecessarily coupling unrelated lifetimes.
3. Read `T: 'a` as “values of `T` may be held for `'a`”; read `T: 'static` as “`T` has no non-`'static` borrows.” Neither implies a particular allocation time or mandatory process-long lifetime.
4. Generic `T` is `Sized` by default. Add `?Sized` only when supporting DSTs through a pointer/reference delivers a verified caller benefit; a struct may have at most one unsized field and it must be the last field; avoid layout/pointer-width assumptions as public ABI.
5. State each implemented trait's actual promise. `From` is infallible and ordinarily preserves semantic value/information; `TryFrom` expresses validation failure. `Borrow` requires equivalent equality/hash/order where used; `AsRef` does not impose that law. Iterator exhaustion need not be permanent unless the API promises fused behavior. Prefer named methods when conversion meaning is ambiguous.
6. Test the public contract, including borrow lifetime, conversion failure, and trait law consequences. Do not test current compiler diagnostics or pointer layout as application behavior.
7. A shared `&T` does not by itself promise physical immutability: `Cell`, `RefCell`, `OnceCell`, caches, reference counts, and synchronization types can mutate internally. Document any consumer-observable mutation, runtime borrow failure, thread-safety boundary, or material cost; prefer ordinary inherited mutability unless interior mutability delivers a named benefit.
8. For a published library, document errors, panic conditions, and unsafe preconditions; choose the compatibility promise before exposing representation. Private fields/newtypes hide representation, `#[non_exhaustive]` reserves expansion, and sealed traits reserve implementation space. Apply each only when its downstream restriction buys a concrete evolution benefit.
9. `Copy` can be derived or manually implemented for eligible types. Its cost depends on representation and optimization; adding/removing it affects the API. `Fn` permits interior mutation and other effects; `FnOnce` consumes the callable, and stronger callable bounds also implement it. A shared reborrow is legitimate while its originating borrow's access restrictions remain satisfied.

## Enforcement

- **Coding agents:** must document the contract and caller benefit in the PR/change record before adding a public trait implementation, `?Sized`, `'static` bound, `Deref`, `AsRef`/`Borrow`, or broad blanket implementation.
- **Review bots:** ask whether a generic type can be a reference, whether a trait law applies, whether `Deref` creates an unwanted implicit API, and whether public interior mutability hides an observable lifecycle/concurrency/failure contract. Do not infer trait-law correctness mechanically.
- **Tests:** retain a consumer-observable regression test only for a meaningful conversion, borrow, comparison/hash, or iteration behavior.

## Exceptions

An unusual API still must uphold its documented trait/safety contracts. Record ergonomic/cost tradeoffs; use named methods if standard-trait semantics do not fit.

## Freshness

Language concepts remain useful, but verify current compiler and standard-library behavior against the Rust Reference and standard-library documentation when changing a public API or supporting a new MSRV.

## Evidence

- [associated items, dyn compatibility, unsafe implementors](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference-traits.md)
- [coercion sites, unsizing, and supertrait upcasting](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference-coercions.md)
- [implementation methods, eligibility, and API evolution](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-copy.md)
- [When to implement From and generic implementations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-from.md)
- [Eq/Ord/Hash equivalence and case-insensitive counterexample](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-borrow.md)
- [interior mutability and synchronization boundaries](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-cell.md)
- [callable hierarchy and consuming receiver](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-fn-once.md)
- [representation guarantees and limitations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference-layout.md)
- [DSTs behind pointers, implicit `Sized` bounds, and a struct's single last unsized field](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference-dynamically-sized-types.md)
- [non_exhaustive downstream construction and matching](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference-type-system-attributes.md)
- [examples, errors, panics, and safety contracts](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-api-guidelines-documentation.md)
- [Outlives, trait-object lifetime defaults, reborrow examples, and blanket/reference implementation overlap, with qualifications](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/pretzelhammer-common-rust-lifetime-misconceptions.md)
- [FusedIterator: continued exhaustion and fuse versus unnecessary bounds](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-fused-iterator.md)
- [restrained Deref, caller-controlled copying, newtypes, sealed traits and private fields](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-api-guidelines-checklist.md)
