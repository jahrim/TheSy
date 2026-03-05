use egg::{Applier, EClass, Id, Pattern, SearchMatches, StopReason, SymbolLang, Var};
use itertools::Itertools;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use smallvec::alloc::fmt::Formatter;
use std::collections::hash_map::RandomState;
use std::rc::Rc;
use std::fmt;
use std::path::Display;
use crate::adapter::{Branch, EGraph, RunnerConfig};
use crate::eggstentions::rewrites::Rewrite;
use crate::eggstentions::searchers::multisearcher::{HasSearch, Searcher};

/// To be used as the op of edges representing potential split
pub const SPLITTER: &'static str = "potential_split";
lazy_static! {
    /// Pattern to find all available splitter edges. Limited arbitrarily to 5 possible splits.
    pub(crate) static ref split_patterns: Vec<Pattern<SymbolLang>> = {
        vec![
            Pattern::from_str(&*format!("({} ?root ?c0 ?c1)", SPLITTER)).unwrap(),
            Pattern::from_str(&*format!("({} ?root ?c0 ?c1 ?c2)", SPLITTER)).unwrap(),
            Pattern::from_str(&*format!("({} ?root ?c0 ?c1 ?c2 ?c3)", SPLITTER)).unwrap(),
            Pattern::from_str(&*format!("({} ?root ?c0 ?c1 ?c2 ?c3 ?c4)", SPLITTER)).unwrap(),
        ]
    };
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct Split {
    root: Id,
    splits: Vec<Id>,
}

impl Split {
    pub fn new(root: Id, splits: Vec<Id>) -> Self {Split{root,splits}}

    pub(crate) fn update<G: EGraph>(&mut self, egraph: &G) {
        self.root = egraph.find(self.root);
        for i in 0..self.splits.len() {
            self.splits[i] = egraph.find(self.splits[i]);
        }
    }
}

impl fmt::Display for Split {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "(root: {}, splits [{}])", self.root, self.splits.iter().map(|x| usize::from(*x).to_string()).intersperse(" ".parse().unwrap()).collect::<String>())
    }
}

pub type SplitApplier<G: EGraph> = Box<dyn FnMut(&mut G, Vec<SearchMatches>) -> Vec<Split>>;

pub struct CaseSplit<G: EGraph> {
    splitter_rules: Vec<(Searcher, SplitApplier<G>)>,
}

impl<G: EGraph> CaseSplit<G> {
    pub fn new(splitter_rules: Vec<(Searcher, SplitApplier<G>)>) -> Self {
        CaseSplit { splitter_rules }
    }

    pub fn from_applier_patterns(
        case_splitters: Vec<(Searcher, Var, Vec<Pattern<SymbolLang>>)>,
    ) -> CaseSplit<G> {
        let res = CaseSplit::new(case_splitters.into_iter().map(|(searcher, root, split_evaluators)| {
            let applier: SplitApplier<G> = Box::new(move |egraph: &mut G, sms: Vec<SearchMatches>| {
                let mut res = vec![];
                for sm in sms {
                    for subst in &sm.substs {
                        res.push(Split::new(subst[root], split_evaluators.iter().map(|ev| { egraph.write_pattern(ev, sm.eclass, &subst)[0] }).collect_vec()));
                    }
                }
                res
            });
            (searcher.clone(), 
            applier)
        }).collect_vec());
        res
    }

    pub fn extend(&mut self, other: Self) {
        self.splitter_rules.extend(other.splitter_rules)
    }

    // CHANGE inlined to call site
    // TODO: can this be an iterator?
    // fn split_graph<'a>(egraph: &'a mut G,
    //                split: &'a Split) -> impl std::iter::Iterator<Item = Branch> + 'a {
    //     split.splits.iter().map(move |child| {
    //         let new_branch = egraph.branch();
    //         egraph.checkout(new_branch);
    //         egraph.union(split.root, *child);
    //         new_branch
    //     })
    // }

    fn equiv_reduction(rules: &[Rewrite], egraph: &mut G, run_depth: usize) {
        let config = RunnerConfig { timeout: Some(Duration::from_secs(60 * 10)), node_limit: Some(egraph.total_number_of_nodes() + 200000), iter_limit: Some(run_depth) };
        match egraph.run(&config, rules).as_ref().unwrap() {
            StopReason::Saturated => {}
            StopReason::IterationLimit(_) => {}
            StopReason::NodeLimit(_) => { warn!("Stopped case split due to node limit") }
            StopReason::TimeLimit(_) => { warn!("Stopped case split due to time limit") }
            StopReason::Other(_) => {}
        };
    }

    pub fn find_splitters(&mut self, egraph: &mut G) -> Vec<Split> {
        let mut res = vec![];
        for (s, c) in &mut self.splitter_rules {
            egraph.rebuild();
            res.extend(c(egraph, s.search(egraph)));
        }
        let f = res.into_iter().unique().collect_vec();
        info!("Found {} splitters", f.len());
        f
    }

    fn merge_conclusions(egraph: &mut G, classes: &Vec<Id>, split_conclusions: Vec<HashMap<Id, Id>>) {
        let mut group_by_splits: HashMap<Vec<Id>, HashSet<Id>> = HashMap::new();
        for c in classes {
            let key = split_conclusions.iter().map(|m| m[c]).collect_vec();
            if !group_by_splits.contains_key(&key) {
                group_by_splits.insert(key.clone(), HashSet::new());
            }
            group_by_splits.get_mut(&key).unwrap().insert(*c);
        }
        for group in group_by_splits.values().filter(|g| g.len() > 1) {
            let first = group.iter().next().unwrap();
            for id in group.iter().dropping(1) {
                egraph.union(*first, *id);
            }
        }
        egraph.rebuild();
    }
    fn collect_merged(egraph: &G, classes: &Vec<Id>) -> HashMap<Id, Id> {
        classes.iter().map(|c| (*c, egraph.find(*c))).collect::<HashMap<Id, Id>>()
    }

    pub fn case_split(&mut self, egraph: &mut G, split_depth: usize, rules: &[Rewrite], run_depth: usize) {
        if !cfg!(feature = "no_split") {
            self.inner_case_split(egraph, split_depth, &Default::default(), rules, run_depth)
        }
    }

    fn inner_case_split(&mut self, egraph: &mut G, split_depth: usize, known_splits: &HashSet<Split>, rules: &[Rewrite], run_depth: usize) {
        if split_depth == 0 {
            return;
        }

        let known_splits: HashSet<Split, RandomState> = known_splits.iter().map(|e| {
                let mut res = e.clone();
                res.update(egraph);
                res
                }).collect();

        let temp = self.find_splitters(egraph);
        let splitters: Vec<&Split> = temp.iter().filter(|s| !known_splits.contains(s)).collect();
        let mut new_known = known_splits.clone();
        new_known.extend(splitters.iter().cloned().cloned());

        let classes = egraph.classes().collect_vec();

        let current_branch = egraph.branch();
        for split in splitters {
            // NOTE this vector can be too big for allocation
            let split_conclusions = split
                .splits
                .iter()
                .map(|child| {
                    let new_branch = egraph.branchout();
                    egraph.checkout(new_branch);
                    egraph.union(split.root, *child);
                    Self::equiv_reduction(rules, egraph, run_depth);
                    self.inner_case_split(egraph, split_depth - 1, &new_known, rules, run_depth);
                    Self::collect_merged(egraph, &classes)
                })
                .collect_vec();
            egraph.checkout(current_branch);
            Self::merge_conclusions(egraph, &classes, split_conclusions);
        }
    }
}
