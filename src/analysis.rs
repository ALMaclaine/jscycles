//! Dense graph IDs and iterative Tarjan/Johnson traversal shared by both APIs.
//!
//! IDs are created only by `intern()`, edges only connect interned IDs, and the
//! graph is immutable during traversal. State vectors have `labels.len()` slots;
//! frame cursors are checked against adjacency lengths before indexing.
#![expect(
    clippy::indexing_slicing,
    reason = "Private dense IDs and checked frame cursors; graph/state invariants are exercised on exhaustive small graphs and deep narrow-stack cases"
)]

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::PathBuf;

use crate::budget::{Meter, Resource, sorting, units};
use crate::graph::{DependencyGraph, PackageDependencyGraph};
use crate::imports::{Import, ImportTarget};

pub(crate) trait Label: Clone + Eq + Hash + Ord {
    fn bytes(&self) -> usize;
}
impl Label for PathBuf {
    fn bytes(&self) -> usize {
        self.as_os_str().as_encoded_bytes().len()
    }
}
impl Label for String {
    fn bytes(&self) -> usize {
        self.len()
    }
}

pub(crate) struct Indexed<K> {
    pub(crate) labels: Vec<K>,
    edges: Vec<Vec<usize>>,
    ids: HashMap<K, usize>,
    pairs: HashSet<(usize, usize)>,
    order: Vec<usize>,
    rank: Vec<usize>,
}

impl<K: Label> Indexed<K> {
    fn new() -> Self {
        Self {
            labels: Vec::new(),
            edges: Vec::new(),
            ids: HashMap::new(),
            pairs: HashSet::new(),
            order: Vec::new(),
            rank: Vec::new(),
        }
    }

    fn admit<M: Meter>(bytes: usize, meter: &M) -> Result<(), M::Error> {
        meter.charge(Resource::Nodes, 1)?;
        meter.charge(Resource::GraphBytes, units(bytes))?;
        meter.charge(Resource::GraphBytes, units(bytes))
    }

    fn insert(&mut self, label: K) -> usize {
        let id = self.labels.len();
        self.labels.push(label.clone());
        let _previous = self.ids.insert(label, id);
        self.edges.push(Vec::new());
        id
    }

    fn intern<M: Meter>(&mut self, label: &K, meter: &M) -> Result<usize, M::Error> {
        meter.charge(Resource::Work, 1)?;
        if let Some(id) = self.ids.get(label) {
            return Ok(*id);
        }
        Self::admit(label.bytes(), meter)?;
        Ok(self.insert(label.clone()))
    }

    #[expect(
        clippy::set_contains_or_insert,
        reason = "The edge quota must admit a new edge before it is retained in the set"
    )]
    fn edge<M: Meter>(&mut self, from: usize, to: usize, meter: &M) -> Result<(), M::Error> {
        meter.charge(Resource::Work, 1)?;
        if !self.pairs.contains(&(from, to)) {
            meter.charge(Resource::Edges, 1)?;
            let _inserted = self.pairs.insert((from, to));
            self.edges[from].push(to);
        }
        Ok(())
    }

    fn finish<M: Meter>(&mut self, meter: &M) -> Result<(), M::Error> {
        sorting(self.labels.len(), meter)?;
        self.order = (0..self.labels.len()).collect();
        self.order.sort_by_key(|id| &self.labels[*id]);
        self.rank = vec![0; self.labels.len()];
        for (rank, id) in self.order.iter().enumerate() {
            self.rank[*id] = rank;
        }
        for neighbors in &mut self.edges {
            sorting(neighbors.len(), meter)?;
            neighbors.sort_by_key(|id| self.rank[*id]);
        }
        Ok(())
    }

    fn retain_cycle<M: Meter>(&self, path: &[usize], meter: &M) -> Result<Vec<usize>, M::Error> {
        meter.charge(Resource::Cycles, 1)?;
        for id in path.iter().chain(path.first()) {
            meter.charge(Resource::Work, 1)?;
            meter.charge(Resource::CycleBytes, units(self.labels[*id].bytes()))?;
            meter.charge(Resource::CycleBytes, units(size_of::<K>()))?;
            meter.charge(Resource::CycleBytes, units(size_of::<usize>()))?;
        }
        let mut cycle = path.to_vec();
        if let Some(first) = path.first() {
            cycle.push(*first);
        }
        Ok(cycle)
    }

    pub(crate) fn label(&self, id: usize) -> &K {
        &self.labels[id]
    }

    pub(crate) fn materialize_path(&self, path: Vec<usize>) -> Vec<K> {
        path.into_iter().map(|id| self.labels[id].clone()).collect()
    }

    pub(crate) fn materialize(&self, paths: Vec<Vec<usize>>) -> Vec<Vec<K>> {
        paths
            .into_iter()
            .map(|path| self.materialize_path(path))
            .collect()
    }

    fn sort_cycles<M: Meter>(&self, paths: &mut [Vec<usize>], meter: &M) -> Result<(), M::Error> {
        if let Some(passes) = paths.len().checked_ilog2() {
            for _ in 0..=passes {
                for path in paths.iter() {
                    meter.charge(Resource::Work, units(path.len()))?;
                }
            }
        }
        paths.sort_by(|a, b| {
            a.iter()
                .map(|id| &self.labels[*id])
                .cmp(b.iter().map(|id| &self.labels[*id]))
        });
        Ok(())
    }
}

impl Indexed<PathBuf> {
    pub(crate) fn files<M: Meter>(graph: &DependencyGraph, meter: &M) -> Result<Self, M::Error> {
        let mut result = Self::new();
        for node in graph.nodes() {
            let from = result.intern(node, meter)?;
            for target in graph.successors(node) {
                let to = result.intern(target, meter)?;
                result.edge(from, to, meter)?;
            }
        }
        result.finish(meter)?;
        Ok(result)
    }

    pub(crate) fn imports<M: Meter>(imports: &[Import], meter: &M) -> Result<Self, M::Error> {
        let mut result = Self::new();
        for import in imports {
            let from = result.intern(&import.source, meter)?;
            if let ImportTarget::Resolved(target) = &import.target {
                let to = result.intern(target, meter)?;
                result.edge(from, to, meter)?;
            }
        }
        result.finish(meter)?;
        Ok(result)
    }
}

impl Indexed<String> {
    fn name<M: Meter>(&mut self, name: &str, meter: &M) -> Result<usize, M::Error> {
        meter.charge(Resource::Work, 1)?;
        if let Some(id) = self.ids.get(name) {
            return Ok(*id);
        }
        Self::admit(name.len(), meter)?;
        Ok(self.insert(name.to_owned()))
    }

    pub(crate) fn packages<M: Meter>(
        graph: &PackageDependencyGraph,
        meter: &M,
    ) -> Result<Self, M::Error> {
        let mut result = Self::new();
        for name in graph.nodes() {
            let from = result.intern(name, meter)?;
            for target in graph.successors(name) {
                let to = result.intern(target, meter)?;
                result.edge(from, to, meter)?;
            }
        }
        result.finish(meter)?;
        Ok(result)
    }
}

pub(crate) struct PackageAnalysis<'a> {
    pub(crate) graph: Indexed<String>,
    pub(crate) evidence: HashMap<(usize, usize), Vec<&'a Import>>,
}

impl<'a> PackageAnalysis<'a> {
    pub(crate) fn build<M: Meter>(
        groups: impl IntoIterator<Item = (&'a str, &'a [Import])>,
        detail: bool,
        meter: &M,
    ) -> Result<Self, M::Error> {
        let mut result = Self {
            graph: Indexed::new(),
            evidence: HashMap::new(),
        };
        for (name, imports) in groups {
            let from = result.graph.name(name, meter)?;
            for import in imports {
                meter.charge(Resource::Work, 1)?;
                if let ImportTarget::WorkspacePackage { package_name, .. } = &import.target {
                    let to = result.graph.name(package_name, meter)?;
                    if from == to {
                        continue;
                    }
                    result.graph.edge(from, to, meter)?;
                    if detail {
                        meter.charge(Resource::GraphBytes, units(size_of::<&Import>()))?;
                        result.evidence.entry((from, to)).or_default().push(import);
                    }
                }
            }
        }
        result.graph.finish(meter)?;
        Ok(result)
    }
}

#[derive(Clone, Copy)]
struct Frame {
    node: usize,
    next: usize,
    found: bool,
}

fn components<K: Label, M: Meter>(
    graph: &Indexed<K>,
    nodes: &[usize],
    meter: &M,
) -> Result<Vec<Vec<usize>>, M::Error> {
    meter.charge(Resource::Work, units(nodes.len()))?;
    let allowed: HashSet<_> = nodes.iter().copied().collect();
    let mut indices = HashMap::new();
    let mut low = HashMap::new();
    let mut on_stack = HashSet::new();
    let mut pending = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut output = Vec::new();
    for root in nodes {
        meter.charge(Resource::Work, 1)?;
        if indices.contains_key(root) {
            continue;
        }
        let root_index = indices.len();
        let _root_old = indices.insert(*root, root_index);
        let _root_low = low.insert(*root, root_index);
        let _root_new = on_stack.insert(*root);
        pending.push(*root);
        frames.push(Frame {
            node: *root,
            next: 0,
            found: false,
        });
        while let Some(frame) = frames.last_mut() {
            meter.charge(Resource::Work, 1)?;
            let node = frame.node;
            if frame.next < graph.edges[node].len() {
                let next = graph.edges[node][frame.next];
                frame.next += 1;
                if !allowed.contains(&next) {
                    continue;
                }
                if let Some(index) = indices.get(&next) {
                    if on_stack.contains(&next) {
                        let value = low[&node].min(*index);
                        let _old = low.insert(node, value);
                    }
                } else {
                    let index = indices.len();
                    let _old = indices.insert(next, index);
                    let _old_low = low.insert(next, index);
                    let _new = on_stack.insert(next);
                    pending.push(next);
                    frames.push(Frame {
                        node: next,
                        next: 0,
                        found: false,
                    });
                }
            } else {
                let _completed = frames.pop();
                if low[&node] == indices[&node] {
                    let mut component = Vec::new();
                    while let Some(top) = pending.pop() {
                        let _removed = on_stack.remove(&top);
                        component.push(top);
                        if top == node {
                            break;
                        }
                    }
                    output.push(component);
                }
                if let Some(parent) = frames.last() {
                    let value = low[&parent.node].min(low[&node]);
                    let _old = low.insert(parent.node, value);
                }
            }
        }
    }
    Ok(output)
}

#[cfg_attr(feature = "profiling", hotpath::measure)]

pub(crate) fn representative<K: Label, M: Meter>(
    graph: &Indexed<K>,
    meter: &M,
) -> Result<Vec<Vec<usize>>, M::Error> {
    let mut output = Vec::new();
    for mut component in components(graph, &graph.order, meter)? {
        sorting(component.len(), meter)?;
        component.sort_by_key(|id| graph.rank[*id]);
        let Some(start) = component.first().copied() else {
            continue;
        };
        if component.len() == 1 {
            if graph.pairs.contains(&(start, start)) {
                output.push(graph.retain_cycle(&[start], meter)?);
            }
            continue;
        }
        let allowed: HashSet<_> = component.into_iter().collect();
        let mut visited = HashSet::from([start]);
        let mut path = vec![start];
        let mut frames = vec![Frame {
            node: start,
            next: 0,
            found: false,
        }];
        while let Some(frame) = frames.last_mut() {
            meter.charge(Resource::Work, 1)?;
            if frame.next == graph.edges[frame.node].len() {
                let _completed = frames.pop();
                let _last = path.pop();
                continue;
            }
            let next = graph.edges[frame.node][frame.next];
            frame.next += 1;
            if !allowed.contains(&next) {
                continue;
            }
            if next == start && path.len() > 1 {
                output.push(graph.retain_cycle(&path, meter)?);
                break;
            }
            if visited.insert(next) {
                path.push(next);
                frames.push(Frame {
                    node: next,
                    next: 0,
                    found: false,
                });
            }
        }
    }
    graph.sort_cycles(&mut output, meter)?;
    Ok(output)
}

fn unblock<M: Meter>(
    node: usize,
    blocked: &mut HashSet<usize>,
    dependencies: &mut HashMap<usize, HashSet<usize>>,
    meter: &M,
) -> Result<(), M::Error> {
    let mut pending = vec![node];
    while let Some(next) = pending.pop() {
        meter.charge(Resource::Work, 1)?;
        if !blocked.remove(&next) {
            continue;
        }
        if let Some(others) = dependencies.remove(&next) {
            for other in others {
                meter.charge(Resource::Work, 1)?;
                if blocked.contains(&other) {
                    pending.push(other);
                }
            }
        }
    }
    Ok(())
}

fn circuit<K: Label, M: Meter>(
    graph: &Indexed<K>,
    component: &[usize],
    meter: &M,
    output: &mut Vec<Vec<usize>>,
) -> Result<(), M::Error> {
    let Some(start) = component.first().copied() else {
        return Ok(());
    };
    let allowed: HashSet<_> = component.iter().copied().collect();
    let mut blocked = HashSet::from([start]);
    let mut dependencies: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut path = vec![start];
    let mut frames = vec![Frame {
        node: start,
        next: 0,
        found: false,
    }];
    while let Some(frame) = frames.last_mut() {
        meter.charge(Resource::Work, 1)?;
        let node = frame.node;
        if frame.next < graph.edges[node].len() {
            let next = graph.edges[node][frame.next];
            frame.next += 1;
            if !allowed.contains(&next) {
                continue;
            }
            if next == start {
                output.push(graph.retain_cycle(&path, meter)?);
                frame.found = true;
            } else if blocked.insert(next) {
                path.push(next);
                frames.push(Frame {
                    node: next,
                    next: 0,
                    found: false,
                });
            }
        } else {
            let found = frame.found;
            let _completed = frames.pop();
            let _last = path.pop();
            if found {
                unblock(node, &mut blocked, &mut dependencies, meter)?;
            } else {
                for next in &graph.edges[node] {
                    meter.charge(Resource::Work, 1)?;
                    if allowed.contains(next) {
                        let _new = dependencies.entry(*next).or_default().insert(node);
                    }
                }
            }
            if let Some(parent) = frames.last_mut() {
                parent.found |= found;
            }
        }
    }
    Ok(())
}

#[cfg_attr(feature = "profiling", hotpath::measure)]

pub(crate) fn elementary<K: Label, M: Meter>(
    graph: &Indexed<K>,
    meter: &M,
) -> Result<Vec<Vec<usize>>, M::Error> {
    let mut output = Vec::new();
    let mut pending = components(graph, &graph.order, meter)?;
    while let Some(mut component) = pending.pop() {
        meter.charge(Resource::Work, 1)?;
        if component.len() < 2 {
            continue;
        }
        sorting(component.len(), meter)?;
        component.sort_by_key(|id| graph.rank[*id]);
        circuit(graph, &component, meter, &mut output)?;
        // Recompute SCCs after removing the start, avoiding exploration of the
        // remaining acyclic suffix of a deep ring. No recursive circuit/unblock.
        if component.len() > 2 {
            pending.extend(components(graph, &component[1..], meter)?);
        }
    }
    graph.sort_cycles(&mut output, meter)?;
    Ok(output)
}
