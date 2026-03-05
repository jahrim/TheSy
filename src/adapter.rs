use crate::egg::{
    CostFunction, ENodeOrVar, Id, Language, Pattern, RecExpr, SearchMatches, StopReason, Subst,
    Symbol, SymbolLang, Var,
};
use crate::eggstentions::{
    appliers::{Applier, HasApplyMatches, HasApplyOne},
    costs::{MinRep, RepOrder},
    rewrites::Rewrite,
    searchers::multisearcher::{HasSearch, HasSearchEClass, HasVars, Searcher},
};
use crate::util::debug::{probe, probe_iterator};
use itertools::Itertools;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;
use std::{collections::HashMap, time::Duration};

pub use common::*;

pub mod common {

    use super::*;

    #[derive(Default)]
    pub struct RunnerConfig {
        pub timeout: Option<Duration>,
        pub node_limit: Option<usize>,
        pub iter_limit: Option<usize>,
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub struct Branch {
        pub id: usize,
    }

    pub trait Searchable {
        fn search(&self, searcher: &Searcher) -> Vec<SearchMatches>;
    }
    pub trait Writable {
        fn write(&mut self, applier: &Applier, eclass: Id, subst: &Subst) -> Vec<Id>;
    }

    pub trait EGraphView: Default + Sized + Debug {
        fn classes(&self) -> impl Iterator<Item = Id> + '_;
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>>;
        fn total_number_of_nodes(&self) -> usize;
        fn lookup(&self, enode: &mut SymbolLang) -> Option<Id>;
        fn find(&self, eclass: Id) -> Id;

        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches>;

        fn extractor(&self) -> impl Extractor<Self>;
        fn branch(&self) -> Branch;
        fn branch_count(&self) -> usize;
        fn backtracking_steps(&self) -> Option<usize> {
            None
        }
    }

    pub trait EGraph: EGraphView {
        fn add(&mut self, enode: SymbolLang) -> Id;
        fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id;
        fn union(&mut self, left: Id, right: Id) -> Id;
        fn rebuild(&mut self);
        fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id>;

        fn write_pattern(
            &mut self,
            applier: &Pattern<SymbolLang>,
            eclass: Id,
            subst: &Subst,
        ) -> Vec<Id>;
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason>;

        fn branchout(&mut self) -> Branch;
        fn checkout(&mut self, branch: Branch);
    }

    impl<G: EGraphView> Searchable for G {
        fn search(&self, searcher: &Searcher) -> Vec<SearchMatches> {
            searcher.search(self)
        }
    }
    impl<G: EGraph> Writable for G {
        fn write(&mut self, applier: &Applier, eclass: Id, subst: &Subst) -> Vec<Id> {
            applier.apply_one(self, eclass, subst)
        }
    }

    pub trait Extractor<G: EGraphView> {
        fn find_best(&mut self, eclass: Id) -> Option<(RepOrder, RecExpr<SymbolLang>)>;
    }

    pub trait Conversion {
        type Left;
        type Right;
        fn conversion(&self, source: &Self::Left) -> Self::Right;
        #[inline(always)]
        fn c(&self, source: &Self::Left) -> Self::Right {
            self.conversion(source)
        }
    }
    pub trait BiConversion: Conversion {
        fn inverse_conversion(&self, source: &Self::Right) -> Self::Left;
        #[inline(always)]
        fn ic(&self, source: &Self::Right) -> Self::Left {
            self.inverse_conversion(source)
        }
    }
}

pub type NoOp = ();
pub mod noop {
    use super::*;

    impl EGraphView for () {
        fn classes(&self) -> impl Iterator<Item = Id> + '_ {
            std::iter::empty()
        }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
            Default::default()
        }
        fn total_number_of_nodes(&self) -> usize {
            Default::default()
        }
        fn lookup(&self, _: &mut SymbolLang) -> Option<Id> {
            None
        }
        fn find(&self, _: Id) -> Id {
            Default::default()
        }

        fn search_pattern(&self, _: &Pattern<SymbolLang>) -> Vec<SearchMatches> {
            Default::default()
        }

        fn extractor(&self) -> impl Extractor<Self> {}

        fn branch(&self) -> Branch {
            Default::default()
        }
        fn branch_count(&self) -> usize {
            Default::default()
        }
    }
    impl EGraph for () {
        fn add(&mut self, _: SymbolLang) -> Id {
            Default::default()
        }
        fn add_expr(&mut self, _: &RecExpr<SymbolLang>) -> Id {
            Default::default()
        }
        fn union(&mut self, _: Id, _: Id) -> Id {
            Default::default()
        }
        fn rebuild(&mut self) {}
        fn equivs(&mut self, _: &RecExpr<SymbolLang>, _: &RecExpr<SymbolLang>) -> Vec<Id> {
            Default::default()
        }

        fn write_pattern(&mut self, _: &Pattern<SymbolLang>, _: Id, _: &Subst) -> Vec<Id> {
            Default::default()
        }
        fn run(&mut self, _: &RunnerConfig, _: &[Rewrite]) -> Option<StopReason> {
            Some(StopReason::Saturated)
        }

        fn checkout(&mut self, _: Branch) {}
        fn branchout(&mut self) -> Branch {
            Default::default()
        }
    }

    impl Extractor<()> for () {
        fn find_best(&mut self, _: Id) -> Option<(RepOrder, RecExpr<SymbolLang>)> {
            Default::default()
        }
    }
}

pub type EasterEgg = easteregg::VersionedEGraph;
pub mod easteregg {
    use crate::{eggstentions::searchers::multisearcher::AsSearcher, probe, probe_iterator};
    use std::{
        fmt::Display,
        ops::{Deref, DerefMut},
        str::FromStr,
    };

    use super::*;
    use easter_egg::ColorId;
    use itertools::Itertools;

    struct ColoredSearcher {
        searcher: Searcher,
        color: ColorId,
    }
    impl Display for ColoredSearcher {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.searcher)
        }
    }
    impl easter_egg::Searcher<easter_egg::SymbolLang, ()> for ColoredSearcher {
        fn search(
            &self,
            egraph: &easter_egg::EGraph<easter_egg::SymbolLang, ()>,
        ) -> Option<easter_egg::SearchMatches> {
            let matches = HasSearch::search(
                &self.searcher,
                &ColoredProjection {
                    egraph,
                    color: self.color,
                },
            );
            let vars = self.vars();
            if matches.is_empty() {
                None
            } else {
                let mut easter_matches = easter_egg::SearchMatches::default();
                for m in matches.into_iter() {
                    let eclass = conversions::id.c(&m.eclass);
                    for subst in m.substs.into_iter() {
                        let subst = conversions::subst {
                            vars: &vars,
                            color: self.color,
                        }
                        .c(&subst);
                        easter_matches
                            .matches
                            .entry(eclass)
                            .or_default()
                            .insert(subst);
                        easter_matches.binds_done += 1;
                    }
                }
                Some(easter_matches)
            }
        }
        fn search_eclass_with_limit(
            &self,
            _: &easter_egg::EGraph<easter_egg::SymbolLang, ()>,
            _: easter_egg::Id,
            _: usize,
        ) -> Option<easter_egg::SearchMatches> {
            unimplemented!("don't think it's needed")
        }
        fn colored_search_eclass_with_limit(
            &self,
            _: &easter_egg::EGraph<easter_egg::SymbolLang, ()>,
            _: easter_egg::Id,
            _: ColorId,
            _: usize,
        ) -> Option<easter_egg::SearchMatches> {
            unimplemented!("don't think it's needed")
        }
        fn vars(&self) -> Vec<easter_egg::Var> {
            self.searcher
                .vars()
                .iter()
                .map(|v| conversions::var.c(v))
                .collect()
        }
    }
    struct ColoredApplier {
        applier: Applier,
        color: ColorId,
    }
    impl Display for ColoredApplier {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.applier)
        }
    }
    impl easter_egg::Applier<easter_egg::SymbolLang, ()> for ColoredApplier {
        fn apply_matches(
            &self,
            egraph: &mut easter_egg::EGraph<easter_egg::SymbolLang, ()>,
            matches: &Option<easter_egg::SearchMatches>,
        ) -> Vec<easter_egg::Id> {
            let mut added = vec![];
            if let Some(mat) = matches {
                for (eclass, substs) in mat.matches.iter() {
                    for subst in substs {
                        let ids = self
                            .apply_one(egraph, *eclass, subst)
                            .into_iter()
                            .filter_map(|id| {
                                let (to, did_something) =
                                    egraph.opt_colored_union(subst.color(), id, *eclass);
                                if did_something {
                                    Some(to)
                                } else {
                                    None
                                }
                            });
                        added.extend(ids)
                    }
                }
            }
            added
        }
        fn apply_one(
            &self,
            egraph: &mut easter_egg::EGraph<easter_egg::SymbolLang, ()>,
            eclass: easter_egg::Id,
            subst: &easter_egg::Subst,
        ) -> Vec<easter_egg::Id> {
            HasApplyOne::apply_one(
                &self.applier,
                &mut ColoredProjection {
                    egraph,
                    color: self.color,
                },
                conversions::id.ic(&eclass),
                &conversions::subst {
                    vars: &self.vars(),
                    color: self.color,
                }
                .ic(subst),
            )
            .iter()
            .map(|c| conversions::id.c(c))
            .collect()
        }
        fn vars(&self) -> Vec<easter_egg::Var> {
            self.applier
                .vars()
                .into_iter()
                .map(|v| conversions::var.c(&v))
                .collect::<Vec<_>>()
        }
    }

    pub mod conversions {
        use super::*;

        #[allow(non_camel_case_types)]
        pub struct id;
        impl Conversion for id {
            type Left = Id;
            type Right = easter_egg::Id;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                easter_egg::Id::from(Into::<usize>::into(*source))
            }
        }
        impl BiConversion for id {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                Id::from(source.0 as usize)
            }
        }
        #[allow(non_camel_case_types)]
        pub struct enode;
        impl Conversion for enode {
            type Left = SymbolLang;
            type Right = easter_egg::SymbolLang;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                easter_egg::SymbolLang {
                    op: easter_egg::Symbol::from(source.op.as_str()),
                    children: source.children.iter().map(|x| id.c(x)).collect(),
                }
            }
        }
        impl BiConversion for enode {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                SymbolLang {
                    op: crate::egg::Symbol::from(source.op.as_str()),
                    children: source.children.iter().map(|x| id.ic(x)).collect(),
                }
            }
        }
        #[allow(non_camel_case_types)]
        pub struct recexpr;
        impl Conversion for recexpr {
            type Left = RecExpr<SymbolLang>;
            type Right = easter_egg::RecExpr<easter_egg::SymbolLang>;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                source.to_string().as_str().parse().unwrap()
            }
        }
        impl BiConversion for recexpr {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                source.to_string().as_str().parse().unwrap()
            }
        }

        #[allow(non_camel_case_types)]
        pub struct var;
        impl Conversion for var {
            type Left = Var;
            type Right = easter_egg::Var;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                source.to_string().as_str().parse().unwrap()
            }
        }
        impl BiConversion for var {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                source.to_string().as_str().parse().unwrap()
            }
        }

        #[allow(non_camel_case_types)]
        pub struct pattern;
        impl Conversion for pattern {
            type Left = Pattern<SymbolLang>;
            type Right = easter_egg::Pattern<easter_egg::SymbolLang>;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                easter_egg::Pattern::from_str(source.to_string().as_str()).unwrap()
            }
        }
        impl BiConversion for pattern {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                Pattern::from_str(source.to_string().as_str()).unwrap()
            }
        }

        #[allow(non_camel_case_types)]
        pub struct subst<'a> {
            pub vars: &'a [easter_egg::Var],
            pub color: easter_egg::ColorId,
        }
        impl Conversion for subst<'_> {
            type Left = Subst;
            type Right = easter_egg::Subst;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                let bindings = self
                    .vars
                    .iter()
                    .filter_map(|v| source.get(conversions::var.ic(v)).map(|value| (v, value)));
                let mut subst: easter_egg::Subst =
                    easter_egg::Subst::colored_with_capacity(self.vars.len(), Some(self.color));
                for (v, value) in bindings {
                    subst.insert(*v, conversions::id.c(value));
                }
                subst
            }
        }
        impl BiConversion for subst<'_> {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                let bindings = self
                    .vars
                    .iter()
                    .filter_map(|v| source.get(*v).map(|value| (v, value)));
                let mut subst: Subst = Subst::with_capacity(self.vars.len());
                for (v, value) in bindings {
                    subst.insert(conversions::var.ic(v), conversions::id.ic(value));
                }
                subst
            }
        }
        #[allow(non_camel_case_types)]
        pub struct matches<'a> {
            pub vars: &'a [easter_egg::Var],
            pub color: easter_egg::ColorId,
        }
        impl Conversion for matches<'_> {
            type Left = SearchMatches;
            type Right = easter_egg::SearchMatches;
            fn conversion(&self, _: &Self::Left) -> Self::Right {
                todo!("not supported yet")
            }
        }
        impl BiConversion for matches<'_> {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                let easter_id = source
                    .matches
                    .first_key_value()
                    .map(|x| x.0)
                    .cloned()
                    .expect("matched eclass");
                let eclass = conversions::id.ic(&easter_id);
                let sconversion = conversions::subst {
                    vars: self.vars,
                    color: self.color,
                };
                let substs = source.matches[&easter_id]
                    .iter()
                    .map(|subst| sconversion.ic(subst))
                    .collect();
                SearchMatches { eclass, substs }
            }
        }
        #[allow(non_camel_case_types)]
        pub struct rw {
            pub color: ColorId,
        }
        impl Conversion for rw {
            type Left = Rewrite;
            type Right = easter_egg::Rewrite<easter_egg::SymbolLang, ()>;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                easter_egg::Rewrite::new(
                    source.name(),
                    ColoredSearcher {
                        searcher: source.searcher.clone(),
                        color: self.color,
                    },
                    ColoredApplier {
                        applier: source.applier.clone(),
                        color: self.color,
                    },
                )
                .unwrap()
            }
        }
        #[allow(non_camel_case_types)]
        pub struct stop_reason;
        impl Conversion for stop_reason {
            type Left = StopReason;
            type Right = easter_egg::StopReason;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                match source {
                    StopReason::Saturated => easter_egg::StopReason::Saturated,
                    StopReason::TimeLimit(l) => easter_egg::StopReason::TimeLimit(*l),
                    StopReason::IterationLimit(l) => easter_egg::StopReason::IterationLimit(*l),
                    StopReason::NodeLimit(l) => easter_egg::StopReason::NodeLimit(*l),
                    StopReason::Other(l) => easter_egg::StopReason::Other(l.clone()),
                }
            }
        }
        impl BiConversion for stop_reason {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                match source {
                    easter_egg::StopReason::Saturated => StopReason::Saturated,
                    easter_egg::StopReason::TimeLimit(l) => StopReason::TimeLimit(*l),
                    easter_egg::StopReason::IterationLimit(l) => StopReason::IterationLimit(*l),
                    easter_egg::StopReason::NodeLimit(l) => StopReason::NodeLimit(*l),
                    easter_egg::StopReason::Other(l) => StopReason::Other(l.clone()),
                }
            }
        }
    }

    #[derive(Debug)]
    struct ColoredProjection<G: Deref<Target = easter_egg::EGraph<easter_egg::SymbolLang, ()>>> {
        egraph: G,
        color: ColorId,
    }
    impl<G: Deref<Target = easter_egg::EGraph<easter_egg::SymbolLang, ()>>> Default
        for ColoredProjection<G>
    {
        fn default() -> Self {
            unimplemented!("should not be called")
        }
    }
    impl<G: Deref<Target = easter_egg::EGraph<easter_egg::SymbolLang, ()>> + Debug> EGraphView
        for ColoredProjection<G>
    {
        fn classes(&self) -> impl Iterator<Item = Id> + '_ {
            self.egraph
                .classes()
                .filter(move |xc| xc.color().is_some_and(|c| c == self.color))
                .map(|xc| conversions::id.ic(&xc.id))
        }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
            self.egraph
                .classes()
                .filter(move |xc| xc.color().is_some_and(|c| c == self.color))
                .map(|xc| {
                    (
                        conversions::id.ic(&xc.id),
                        xc.nodes
                            .iter()
                            .map(|xn| conversions::enode.ic(xn))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
        }
        fn total_number_of_nodes(&self) -> usize {
            // TODO this has better performances but is overestimating the number of enodes in the
            //      current branch
            self.egraph.total_number_of_nodes()
        }
        fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> {
            self.egraph
                .colored_lookup(self.color, conversions::enode.c(enode))
                .map(|x| conversions::id.ic(&x))
        }
        fn find(&self, eclass: Id) -> Id {
            conversions::id.ic(&self
                .egraph
                .colored_find(self.color, conversions::id.c(&eclass)))
        }
        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> {
            // NOTE easteregg complains about searching a dirty e-graph.
            // This ensures that the egraph is not dirty.
            let searcher = conversions::pattern.c(searcher);
            let mconversion = conversions::matches {
                vars: &searcher.vars(),
                color: self.color,
            };
            let colors = self.egraph.get_colors_parents(self.color);
            let ms = self
                .egraph
                .classes()
                .filter(move |xc| {
                    xc.color()
                        .is_some_and(|c| c == self.color || colors.contains(&c))
                })
                .filter_map(|eclass| {
                    use easter_egg::Searcher;
                    searcher
                        .search_eclass(&self.egraph, eclass.id)
                        .map(|m| mconversion.ic(&m))
                })
                .unique_by(|m| m.eclass)
                .collect();
            ms
        }
        fn extractor(&self) -> impl Extractor<Self> {
            easter_egg::Extractor::new(&self.egraph, MinRep)
        }
        fn branch(&self) -> Branch {
            unimplemented!("Cannot branch from projection")
        }
        fn branch_count(&self) -> usize {
            unimplemented!("Cannot branch from projection")
        }
    }
    impl<G: DerefMut<Target = easter_egg::EGraph<easter_egg::SymbolLang, ()>> + Debug> EGraph
        for ColoredProjection<G>
    {
        fn add(&mut self, enode: SymbolLang) -> Id {
            conversions::id.ic(&self
                .egraph
                .colored_add(self.color, conversions::enode.c(&enode)))
        }
        fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id {
            conversions::id.ic(&self
                .egraph
                .colored_add_expr(self.color, &conversions::recexpr.c(expr)))
        }
        fn union(&mut self, left: Id, right: Id) -> Id {
            conversions::id.ic(&self
                .egraph
                .colored_union(
                    self.color,
                    conversions::id.c(&left),
                    conversions::id.c(&right),
                )
                .0)
        }
        fn rebuild(&mut self) {
            self.egraph.rebuild();
        }
        fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> {
            self.rebuild();
            let matches1 = easter_egg::Searcher::search(
                &ColoredSearcher {
                    searcher: Pattern::from(left.as_ref()).as_searcher(),
                    color: self.color,
                },
                &self.egraph,
            );
            trace!("Matches1 ({:?}): {:?}", left, matches1);

            let matches2 = easter_egg::Searcher::search(
                &ColoredSearcher {
                    searcher: Pattern::from(right.as_ref()).as_searcher(),
                    color: self.color,
                },
                &self.egraph,
            );
            trace!("Matches2 ({:?}): {:?}", right, matches2);

            let mut equiv_eclasses = Vec::new();

            if let Some(m1) = &matches1 {
                for (m1_eclass, m1_subs) in &m1.matches {
                    let m1_eclass = conversions::id.ic(m1_eclass);
                    // if m1_subs.iter().all(|s| s.color().is_some()) {
                    //     continue;
                    // }
                    if let Some(m2) = &matches2 {
                        for (eclass, subs) in &m2.matches {
                            let eclass = conversions::id.ic(eclass);
                            // if subs.iter().all(|s| s.color().is_some()) {
                            //     continue;
                            // }
                            if self.find(m1_eclass) == self.find(eclass) {
                                equiv_eclasses.push(m1_eclass)
                            }
                        }
                    }
                }
            }
            equiv_eclasses
        }
        fn write_pattern(
            &mut self,
            applier: &Pattern<SymbolLang>,
            eclass: Id,
            subst: &Subst,
        ) -> Vec<Id> {
            let applier = conversions::pattern.c(applier);
            let eclass = conversions::id.c(&eclass);
            let subst = conversions::subst {
                vars: &applier.vars(),
                color: self.color,
            }
            .c(subst);
            easter_egg::Applier::apply_one(&applier, &mut self.egraph, eclass, &subst)
                .iter()
                .map(|x| conversions::id.ic(x))
                .collect()
        }
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> {
            fn check_time(
                result: &mut Result<(), StopReason>,
                start_time: Instant,
                config: &RunnerConfig,
            ) {
                if let Some(timeout) = config.timeout {
                    let elapsed = start_time.elapsed();
                    if elapsed > timeout {
                        *result = Err(StopReason::TimeLimit(elapsed.as_secs_f64()));
                    }
                }
            }
            fn check_nodes<G: Deref<Target = easter_egg::EGraph<easter_egg::SymbolLang, ()>>>(
                result: &mut Result<(), StopReason>,
                egraph: &G,
                config: &RunnerConfig,
            ) {
                if let Some(node_limit) = config.node_limit {
                    let size = egraph.total_number_of_nodes();
                    if size > node_limit {
                        *result = Err(StopReason::NodeLimit(size));
                    }
                }
            }
            fn check_iters(
                result: &mut Result<(), StopReason>,
                iterations: usize,
                config: &RunnerConfig,
            ) {
                if let Some(iter_limit) = config.iter_limit {
                    if iterations > iter_limit {
                        *result = Err(StopReason::IterationLimit(iterations));
                    }
                }
            }

            let rules = rules.iter().collect::<Vec<_>>();
            let mut iterations: usize = 0;
            let mut result: Result<(), StopReason> = Ok(());
            let start_time: Instant = Instant::now();
            loop {
                self.rebuild();
                if result.is_ok() {
                    check_time(&mut result, start_time, config);
                    check_nodes(&mut result, &self.egraph, config);
                    check_iters(&mut result, iterations, config);
                }
                if let Err(stop_reason) = result {
                    return Some(stop_reason);
                }

                let mut matches: Vec<Vec<SearchMatches>> = Vec::new();
                for rule in rules.iter() {
                    matches.push(rule.searcher.search(self));
                    check_time(&mut result, start_time, config);
                    if let Err(timeout) = result {
                        return Some(timeout);
                    }
                }

                let mut applications: HashMap<&String, usize> = HashMap::new();
                for (rule, matched) in rules.iter().zip(matches) {
                    let new_applications = rule.applier.apply_matches(self, &matched).len();
                    if new_applications > 0 {
                        *applications.entry(rule.name()).or_default() += new_applications;
                    }
                    check_time(&mut result, start_time, config);
                    if let Err(timeout) = result {
                        return Some(timeout);
                    }
                }

                if applications.is_empty() {
                    result = Err(StopReason::Saturated)
                }
                iterations += 1;
            }
        }
        // NOTE below is saturation with easteregg's BackoffScheduler, reducing the number of rewrites
        // applied. We use the one above, because we currently do not support rewrite scheduling.
        // -----------------------------------------------------------------------------------------
        // fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> {
        //     // NOTE Where are colors here?
        //     // The checkout color is used by the `Searcher`s, which rely on `Pattern`s, which rely
        //     // on `search_pattern` (above), which rely on the current checkout.
        //     let mut runner =
        //         easter_egg::Runner::default().with_egraph(std::mem::take(&mut self.egraph));
        //     if let Some(timeout) = config.timeout {
        //         runner = runner.with_time_limit(timeout)
        //     }
        //     if let Some(node_limit) = config.node_limit {
        //         runner = runner.with_node_limit(node_limit)
        //     }
        //     if let Some(iter_limit) = config.iter_limit {
        //         runner = runner.with_iter_limit(iter_limit)
        //     }
        //     runner = runner.run(
        //         &rules
        //             .iter()
        //             .map(|r| conversions::rw { color: self.color }.c(r))
        //             .collect::<Vec<_>>(),
        //     );
        //     runner.egraph.rebuild();
        //     *self.egraph = runner.egraph;
        //     runner.stop_reason.map(|x| conversions::stop_reason.ic(&x))
        // }
        fn branchout(&mut self) -> Branch {
            unimplemented!("[branchout] not supported")
        }
        fn checkout(&mut self, _: Branch) {
            unimplemented!("[checkout] not supported")
        }
    }

    #[derive(Debug)]
    pub struct VersionedEGraph {
        egraph: easter_egg::EGraph<easter_egg::SymbolLang, ()>,
        branches: Vec<easter_egg::ColorId>,
        checkout: Branch,
    }
    impl Default for VersionedEGraph {
        fn default() -> Self {
            let mut easteregg = VersionedEGraph {
                egraph: Default::default(),
                branches: vec![],
                checkout: Branch { id: 0 },
            };
            let root = easteregg.egraph.create_color(None);
            easteregg.branches.push(root);
            easteregg.checkout(easteregg.checkout);
            easteregg
        }
    }
    impl VersionedEGraph {
        fn project(&self) -> ColoredProjection<&easter_egg::EGraph<easter_egg::SymbolLang, ()>> {
            ColoredProjection {
                egraph: &self.egraph,
                color: self.branches[self.checkout.id],
            }
        }
        fn project_mut(
            &mut self,
        ) -> ColoredProjection<&mut easter_egg::EGraph<easter_egg::SymbolLang, ()>> {
            ColoredProjection {
                egraph: &mut self.egraph,
                color: self.branches[self.checkout.id],
            }
        }
    }
    impl EGraphView for VersionedEGraph {
        fn classes(&self) -> impl Iterator<Item = Id> + '_ {
            // Had to inline the code, because of lifetime problems with `self.project().classes()`
            let color = self.branches[self.checkout.id];
            self.egraph
                .classes()
                .filter(move |xc| xc.color().is_some_and(|c| c == color))
                .map(|xc| conversions::id.ic(&xc.id))
        }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
            self.project().enodes()
        }
        fn total_number_of_nodes(&self) -> usize {
            self.project().total_number_of_nodes()
        }
        fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> {
            self.project().lookup(enode)
        }
        fn find(&self, eclass: Id) -> Id {
            self.project().find(eclass)
        }
        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> {
            self.project().search_pattern(searcher)
        }
        fn extractor(&self) -> impl Extractor<Self> {
            easter_egg::Extractor::new(&self.egraph, MinRep)
        }
        fn branch(&self) -> Branch {
            self.checkout
        }
        fn branch_count(&self) -> usize {
            self.branches.len()
        }
    }
    impl EGraph for VersionedEGraph {
        fn add(&mut self, enode: SymbolLang) -> Id {
            self.project_mut().add(enode)
        }
        fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id {
            self.project_mut().add_expr(expr)
        }
        fn union(&mut self, left: Id, right: Id) -> Id {
            self.project_mut().union(left, right)
        }
        fn rebuild(&mut self) {
            self.project_mut().rebuild()
        }
        fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> {
            self.project_mut().equivs(left, right)
        }
        fn write_pattern(
            &mut self,
            applier: &Pattern<SymbolLang>,
            eclass: Id,
            subst: &Subst,
        ) -> Vec<Id> {
            self.project_mut().write_pattern(applier, eclass, subst)
        }
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> {
            self.project_mut().run(config, rules)
        }
        fn branchout(&mut self) -> Branch {
            let color = self.branches[self.checkout.id];
            let new_color = self.egraph.create_color(Some(color));
            self.branches.push(new_color);
            Branch {
                id: self.branches.len() - 1,
            }
        }
        fn checkout(&mut self, branch: Branch) {
            self.checkout = branch
        }
    }

    impl easter_egg::CostFunction<easter_egg::SymbolLang> for MinRep {
        type Cost = RepOrder;
        fn cost<C>(&mut self, enode: &easter_egg::SymbolLang, mut costs: C) -> Self::Cost
        where
            C: FnMut(easter_egg::Id) -> Self::Cost,
        {
            CostFunction::cost(self, &conversions::enode.ic(enode), |id| {
                costs(conversions::id.c(&id))
            })
        }
    }
    impl<G> Extractor<ColoredProjection<G>>
        for easter_egg::Extractor<'_, MinRep, easter_egg::SymbolLang, ()>
    where
        G: Deref<Target = easter_egg::EGraph<easter_egg::SymbolLang, ()>> + Debug,
    {
        fn find_best(&mut self, eclass: Id) -> Option<(RepOrder, RecExpr<SymbolLang>)> {
            // NOTE Where are colors here?
            // Colors are bound to eclasses (e.g. same term can have multiple colored eclasses),
            // so find_best already keeps track of colors.
            Some(easter_egg::Extractor::find_best(
                self,
                conversions::id.c(&eclass),
            ))
            .map(|(x, y)| (x, conversions::recexpr.ic(&y)))
        }
    }
    impl Extractor<EasterEgg> for easter_egg::Extractor<'_, MinRep, easter_egg::SymbolLang, ()> {
        fn find_best(&mut self, eclass: Id) -> Option<(RepOrder, RecExpr<SymbolLang>)> {
            // NOTE Where are colors here?
            // Colors are bound to eclasses (e.g. same term can have multiple colored eclasses),
            // so find_best already keeps track of colors.
            Some(easter_egg::Extractor::find_best(
                self,
                conversions::id.c(&eclass),
            ))
            .map(|(x, y)| (x, conversions::recexpr.ic(&y)))
        }
    }
}

pub type Veg = veg::VersionedEGraph;
pub mod veg {
    use super::*;
    use crate::probe;
    use crate::veg::structures::egraph::versioned::VersionedEGraph as _;
    use crate::veg::structures::egraph::{EGraph as _, EGraphView as _};

    pub mod ext {
        pub use crate::veg::structures::egraph::ematching::machine;
        pub use crate::veg::structures::egraph::ematching::naive::*;
        pub use crate::veg::structures::egraph::*;
        pub use crate::veg::structures::versiontree::*;
        pub use crate::veg::util::id::*;
    }
    pub mod conversions {
        use super::*;
        use std::marker::PhantomData;

        #[allow(non_camel_case_types)]
        pub struct id;
        impl Conversion for id {
            type Left = Id;
            type Right = ext::EClass;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                unsafe { std::mem::transmute::<Id, ext::EClass>(*source) }
            }
        }
        impl BiConversion for id {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                unsafe { std::mem::transmute::<ext::EClass, Id>(*source) }
            }
        }
        #[allow(non_camel_case_types)]
        pub struct op;
        impl Conversion for op {
            type Left = crate::egg::Symbol;
            type Right = ext::Operator;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                ext::SymbolIds::from_string(source.as_str())
            }
        }
        impl BiConversion for op {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                crate::egg::Symbol::from(ext::SymbolIds::to_string(*source).unwrap())
            }
        }
        #[allow(non_camel_case_types)]
        pub struct enode;
        impl Conversion for enode {
            type Left = SymbolLang;
            type Right = ext::ENode;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                ext::ENode::application(
                    conversions::op.c(&source.op),
                    source
                        .children
                        .iter()
                        .map(|arg| conversions::id.c(arg))
                        .collect(),
                )
            }
        }
        impl BiConversion for enode {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                SymbolLang::new(
                    conversions::op.ic(&source.op),
                    source
                        .args
                        .iter()
                        .map(|arg| conversions::id.ic(arg))
                        .collect(),
                )
            }
        }
        #[allow(non_camel_case_types)]
        pub struct recexpr<L: Language> {
            phantom: PhantomData<L>,
        }
        impl<L: Language> recexpr<L> {
            pub fn new() -> recexpr<L> {
                recexpr {
                    phantom: Default::default(),
                }
            }
        }
        impl<L: Language> Conversion for recexpr<L> {
            type Left = RecExpr<L>;
            type Right = Vec<L>;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                unsafe { std::mem::transmute::<&RecExpr<L>, &Vec<L>>(source) }.clone()
            }
        }
        impl<L: Language> BiConversion for recexpr<L> {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                let mut expr: RecExpr<L> = Default::default();
                for x in source {
                    expr.add(x.clone());
                }
                expr
            }
        }

        #[allow(non_camel_case_types)]
        pub struct var;
        impl Conversion for var {
            type Left = Var;
            type Right = ext::query::Var;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                let var_id = unsafe { std::mem::transmute::<Var, u32>(*source) } as usize;
                ext::query::Var { id: var_id.into() }
            }
        }
        impl BiConversion for var {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                unsafe { std::mem::transmute::<u32, Var>(Into::<usize>::into(source.id) as u32) }
            }
        }
        #[allow(non_camel_case_types)]
        pub struct mvar;
        impl Conversion for mvar {
            type Left = Var;
            type Right = ext::machine::Var;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                source.to_string().as_str().parse().unwrap()
            }
        }
        impl BiConversion for mvar {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                source.to_string().as_str().parse().unwrap()
            }
        }

        #[allow(non_camel_case_types)]
        pub struct pattern;
        impl Conversion for pattern {
            type Left = Pattern<SymbolLang>;
            type Right = ext::query::Query;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                let terms: Vec<ENodeOrVar<SymbolLang>> =
                    conversions::recexpr::<ENodeOrVar<SymbolLang>>::new().c(&source.ast);
                let mut queries: Vec<ext::query::Query> = vec![];
                for term in terms {
                    match term {
                        ENodeOrVar::Var(v) => {
                            let query = ext::query::Query::Var(conversions::var.c(&v));
                            queries.push(query)
                        }
                        ENodeOrVar::ENode(xn) => {
                            let pat = ext::query::Pattern {
                                op: conversions::op.c(&xn.op),
                                subqueries: xn
                                    .children
                                    .iter()
                                    .map(|arg| queries[Into::<usize>::into(*arg)].clone())
                                    .collect(),
                            };
                            let query = ext::query::Query::Pattern(pat);
                            queries.push(query);
                        }
                    }
                }
                match queries.pop() {
                    Some(query) => query,
                    any => panic!("conversions::pattern: {:?} should never happen", any),
                }
            }
        }

        #[allow(non_camel_case_types)]
        pub struct mpattern;
        impl Conversion for mpattern {
            type Left = Pattern<SymbolLang>;
            type Right = ext::machine::Pattern<ext::ENode>;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                // NOTE this conversion is very expensive and only required because we are not
                // reimplementing TheSy to use our types directly. Most of the expense comes from
                // `write_pattern`. Additionally, the number of conversions done in the tests is
                // non-deterministic and highly-variable (probably due to iterating over a set or
                // map somewhere).
                ext::machine::Pattern::<ext::ENode>::from_str(source.to_string().as_str()).unwrap()
            }
        }

        #[allow(non_camel_case_types)]
        pub struct subst;
        impl Conversion for subst {
            type Left = Subst;
            type Right = ext::ematching::naive::engine::Binding;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                let mut bindings = ext::ematching::naive::engine::Binding::default();
                let source = unsafe {
                    std::mem::transmute::<&Subst, &smallvec::SmallVec<[(Var, Id); 3]>>(source)
                };
                for (v, value) in source {
                    bindings.bind(&conversions::var.c(&v), conversions::id.c(&value));
                }
                bindings
            }
        }
        impl BiConversion for subst {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                let mut substs = Subst::with_capacity(source.len());
                for (v, value) in source.assignments.iter() {
                    substs.insert(conversions::var.ic(v), conversions::id.ic(value));
                }
                substs
            }
        }

        #[allow(non_camel_case_types)]
        pub struct msubst;
        impl Conversion for msubst {
            type Left = Subst;
            type Right = ext::machine::Subst;
            fn conversion(&self, source: &Self::Left) -> Self::Right {
                let source = unsafe {
                    std::mem::transmute::<&Subst, &smallvec::SmallVec<[(Var, Id); 3]>>(source)
                };
                let mut substs = ext::machine::Subst::with_capacity(source.capacity());
                for (v, value) in source {
                    substs.insert(conversions::mvar.c(&v), conversions::id.c(&value));
                }
                substs
            }
        }
        impl BiConversion for msubst {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                let source = unsafe {
                    std::mem::transmute::<
                        &ext::machine::Subst,
                        &smallvec::SmallVec<[(ext::machine::Var, ext::EClass); 3]>,
                    >(source)
                };
                let mut substs = Subst::with_capacity(source.capacity());
                for (v, value) in source {
                    substs.insert(conversions::mvar.ic(&v), conversions::id.ic(&value));
                }
                substs
            }
        }

        #[allow(non_camel_case_types)]
        pub struct matches;
        impl Conversion for matches {
            type Left = Vec<SearchMatches>;
            type Right = Vec<ext::ematching::naive::engine::SearchResult>;
            fn conversion(&self, _: &Self::Left) -> Self::Right {
                todo!("not supported yet")
            }
        }
        impl BiConversion for matches {
            fn inverse_conversion(&self, source: &Self::Right) -> Self::Left {
                source
                    .iter()
                    .map(|m| (m.eclass, conversions::subst.ic(&m.binding)))
                    .into_group_map()
                    .into_iter()
                    .map(|(eclass, substs)| SearchMatches {
                        eclass: conversions::id.ic(&eclass),
                        substs,
                    })
                    .collect()
            }
        }
    }

    #[derive(Debug)]
    pub struct VersionedEGraph {
        egraph: ext::versioned::basic::VersionedEGraph<()>,
        branches: Vec<ext::Version>,
        checkout: Branch,
        last_modified: Option<Branch>,
        backtracking_steps: usize,
    }
    impl Default for VersionedEGraph {
        fn default() -> Self {
            let mut veg = VersionedEGraph {
                egraph: ext::versioned::basic::VersionedEGraph::<()>::with_ematching_caches(10),
                branches: vec![ext::VersionTree::ROOT_VERSION],
                checkout: Branch { id: 0 },
                last_modified: None,
                backtracking_steps: 0,
            };
            veg.egraph.flags_mut().lock_superversions = false;
            veg.egraph.flags_mut().propagate = true;
            veg.checkout(veg.checkout);
            veg
        }
    }
    impl VersionedEGraph {
        #[inline(always)]
        fn version(&self) -> ext::Version {
            self.branches[self.checkout.id]
        }
        fn proj_mut(
            &mut self,
        ) -> ext::versioned::basic::Projection<(), &mut ext::versioned::basic::VersionedEGraph<()>>
        {
            if let Some(last_modified) = self.last_modified {
                if self
                    .egraph
                    .versioning()
                    .ancestors(last_modified.id)
                    .any(|v| v == self.version())
                {
                    self.backtracking_steps += 1;
                }
            }
            self.last_modified = Some(self.checkout);
            self.egraph.take(self.version())
        }
    }

    impl EGraphView for VersionedEGraph {
        fn backtracking_steps(&self) -> Option<usize> {
            Some(self.backtracking_steps)
        }
        fn classes(&self) -> impl Iterator<Item = Id> {
            // TODO Avoid cloning: for now cloning is required because of
            // borrow-checking problems with projections
            use ext::machine::MachineRequirements;
            self.egraph
                .focus(self.version())
                .canonicals()
                .map(|xc| conversions::id.ic(&xc))
                .collect::<Vec<_>>()
                .into_iter()
        }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
            self.egraph
                .focus(self.version())
                .universe()
                .map(|xc| {
                    (
                        conversions::id.ic(&xc),
                        conversions::enode.ic(self.egraph.get_enode(xc)),
                    )
                })
                .into_group_map()
        }
        fn total_number_of_nodes(&self) -> usize {
            self.egraph
                .focus(self.version())
                .universe()
                .map(|xc| {
                    (
                        conversions::id.ic(&xc),
                        conversions::enode.ic(self.egraph.get_enode(xc)),
                    )
                })
                .count()
        }
        fn lookup(&self, x: &mut SymbolLang) -> Option<Id> {
            self.egraph
                .focus(self.version())
                .get_class(&conversions::enode.c(x))
                .map(|xc| conversions::id.ic(xc))
        }
        fn find(&self, x: Id) -> Id {
            conversions::id.ic(&self
                .egraph
                .focus(self.version())
                .find(conversions::id.c(&x)))
        }

        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> {
            let searcher = conversions::mpattern.c(searcher);
            let r = ext::machine::CanEMatch::ematch(&searcher, &self.egraph.focus(self.version()))
                .into_iter()
                .enumerate()
                .map(|(i, m)| {
                    let result = SearchMatches {
                        eclass: conversions::id.ic(&m.eclass),
                        substs: m
                            .substs
                            .into_iter()
                            .map(|s| conversions::msubst.ic(&s))
                            .collect(),
                    };
                    result
                })
                .collect();
            r
        }

        fn extractor(&self) -> impl Extractor<Self> {
            VegExtractor::new(self)
        }

        fn branch(&self) -> Branch {
            self.checkout
        }
        fn branch_count(&self) -> usize {
            self.branches.len()
        }
    }
    impl EGraph for VersionedEGraph {
        fn add(&mut self, xn: SymbolLang) -> Id {
            let xc: ext::EClass = self.proj_mut().add(conversions::enode.c(&xn));
            conversions::id.ic(&xc)
        }
        fn add_expr(&mut self, recx: &RecExpr<SymbolLang>) -> Id {
            let mut cs: Vec<ext::EClass> = vec![];
            let mut proj = self.proj_mut();
            for xn in conversions::recexpr::<SymbolLang>::new().c(recx) {
                cs.push(
                    proj.add(ext::ENode::application(
                        conversions::op.c(&xn.op),
                        xn.children
                            .iter()
                            .map(|arg| cs[Into::<usize>::into(*arg)])
                            .collect(),
                    )),
                );
            }
            conversions::id.ic(cs.last().unwrap())
        }
        fn union(&mut self, x: Id, y: Id) -> Id {
            conversions::id.ic(&self
                .proj_mut()
                .union(conversions::id.c(&x), conversions::id.c(&y)))
        }
        fn rebuild(&mut self) {
            self.proj_mut().rebuild()
        }
        fn equivs(&mut self, recx: &RecExpr<SymbolLang>, recy: &RecExpr<SymbolLang>) -> Vec<Id> {
            fn lookup_expr<G: ext::EGraph>(
                egraph: &mut G,
                recx: &RecExpr<SymbolLang>,
            ) -> Option<ext::EClass> {
                let mut cs: Vec<ext::EClass> = vec![];
                for xn in conversions::recexpr::<SymbolLang>::new().c(recx) {
                    let xc = egraph.get_class(&ext::ENode::application(
                        conversions::op.c(&xn.op),
                        xn.children
                            .iter()
                            .map(|arg| cs[Into::<usize>::into(*arg)])
                            .collect(),
                    ));
                    if let Some(xc) = xc {
                        cs.push(egraph.find(*xc));
                    } else {
                        return None;
                    }
                }
                cs.last().cloned()
            }
            let mut proj = self.proj_mut();
            let xc = lookup_expr(&mut proj, recx);
            let yc = lookup_expr(&mut proj, recy);
            xc.filter(|xc| yc.is_some_and(|yc| *xc == yc))
                .map(|xc| vec![conversions::id.ic(&xc)])
                .unwrap_or_default()
        }

        fn write_pattern(
            &mut self,
            applier: &Pattern<SymbolLang>,
            eclass: Id,
            subst: &Subst,
        ) -> Vec<Id> {
            let applier = conversions::mpattern.c(applier);
            let eclass = conversions::id.c(&eclass);
            let subst = conversions::msubst.c(subst);
            let mut result =
                ext::machine::CanBind::bind(&applier, &mut self.proj_mut(), eclass, &subst)
                    .into_iter()
                    .map(|c| conversions::id.ic(&c))
                    .collect::<Vec<_>>();
            if result.is_empty() {
                result.push(conversions::id.ic(&eclass));
            }
            result
        }
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> {
            fn check_time(
                result: &mut Result<(), StopReason>,
                start_time: Instant,
                config: &RunnerConfig,
            ) {
                if let Some(timeout) = config.timeout {
                    let elapsed = start_time.elapsed();
                    if elapsed > timeout {
                        *result = Err(StopReason::TimeLimit(elapsed.as_secs_f64()));
                    }
                }
            }
            fn check_nodes<G: ext::EGraphView>(
                result: &mut Result<(), StopReason>,
                egraph: &G,
                config: &RunnerConfig,
            ) {
                if let Some(node_limit) = config.node_limit {
                    let size = egraph.universe().count();
                    if size > node_limit {
                        *result = Err(StopReason::NodeLimit(size));
                    }
                }
            }
            fn check_iters(
                result: &mut Result<(), StopReason>,
                iterations: usize,
                config: &RunnerConfig,
            ) {
                if let Some(iter_limit) = config.iter_limit {
                    if iterations > iter_limit {
                        *result = Err(StopReason::IterationLimit(iterations));
                    }
                }
            }

            let rules = rules.iter().collect::<Vec<_>>();
            let mut iterations: usize = 0;
            let mut result: Result<(), StopReason> = Ok(());
            let start_time: Instant = Instant::now();
            loop {
                self.rebuild();

                if result.is_ok() {
                    check_time(&mut result, start_time, config);
                    check_nodes(&mut result, &self.egraph.focus(self.version()), config);
                    check_iters(&mut result, iterations, config);
                }
                if let Err(stop_reason) = result {
                    return Some(stop_reason);
                }

                let mut matches: Vec<Vec<SearchMatches>> = Vec::new();
                for rule in rules.iter() {
                    matches.push(rule.searcher.search(self));
                    check_time(&mut result, start_time, config);
                    if let Err(timeout) = result {
                        return Some(timeout);
                    }
                }

                let mut applications: HashMap<&String, usize> = HashMap::new();
                for (rule, matched) in rules.iter().zip(matches) {
                    let new_applications = rule.applier.apply_matches(self, &matched).len();
                    if new_applications > 0 {
                        *applications.entry(rule.name()).or_default() += new_applications;
                    }
                    check_time(&mut result, start_time, config);
                    if let Err(timeout) = result {
                        return Some(timeout);
                    }
                }

                if applications.is_empty() {
                    result = Err(StopReason::Saturated)
                }
                iterations += 1;
            }
        }

        fn checkout(&mut self, checkout: Branch) {
            self.checkout = checkout;
        }
        fn branchout(&mut self) -> Branch {
            let new_version = self.egraph.branchout(self.version());
            self.branches.push(new_version);
            Branch {
                id: self.branches.len() - 1,
            }
        }
    }
    #[derive(Debug)]
    pub struct VegExtractor<C> {
        enodes_cache: HashMap<Id, Vec<SymbolLang>>,
        cost_cache: HashMap<Id, (SymbolLang, C)>,
    }
    impl<C> VegExtractor<C> {
        pub fn new<G: EGraphView>(egraph: &G) -> VegExtractor<C> {
            VegExtractor {
                enodes_cache: egraph.enodes(),
                cost_cache: Default::default(),
            }
        }
    }
    impl<G: EGraphView> Extractor<G> for VegExtractor<RepOrder> {
        fn find_best(&mut self, eclass: Id) -> Option<(RepOrder, RecExpr<SymbolLang>)> {
            fn ecost<'a>(
                eclass: Id,
                enodes: &HashMap<Id, Vec<SymbolLang>>,
                cache: &'a mut HashMap<Id, (SymbolLang, RepOrder)>,
            ) -> &'a (SymbolLang, RepOrder) {
                let members: &Vec<SymbolLang> = &enodes[&eclass];
                let mut min_node = &members[0];
                let mut class_cost: RepOrder = cost(min_node, enodes, cache);
                for member in members.iter().skip(1) {
                    let member_cost = cost(member, enodes, cache);
                    // Greedy Strategy
                    if member_cost < class_cost {
                        class_cost = member_cost;
                        min_node = member;
                    }
                }
                cache
                    .entry(eclass)
                    .or_insert((min_node.clone(), class_cost))
            }
            fn cost(
                enode: &SymbolLang,
                enodes: &HashMap<Id, Vec<SymbolLang>>,
                cache: &mut HashMap<Id, (SymbolLang, RepOrder)>,
            ) -> RepOrder {
                let mut depth: usize = 0;
                let mut size: usize = 1;
                let mut vars: Vec<String> = vec![];
                for arg in enode.children.iter() {
                    let arg_cost = match cache.get(arg) {
                        Some(cost) => cost,
                        None => ecost(*arg, enodes, cache),
                    };
                    depth = std::cmp::max(depth, arg_cost.1.depth);
                    size += arg_cost.1.size;
                    vars.extend(arg_cost.1.vars.iter().cloned());
                }
                if enode.op.as_str().starts_with("ts_ph") {
                    vars.push(enode.op.to_string());
                }
                RepOrder {
                    depth: depth + 1,
                    size,
                    vars,
                }
            }
            fn build_rec_expr(
                eclass: Id,
                cache: &HashMap<Id, (SymbolLang, RepOrder)>,
                expr: &mut RecExpr<SymbolLang>,
            ) -> Id {
                let mut best_node = cache[&eclass].0.clone();
                for arg in best_node.children.iter_mut() {
                    let id = build_rec_expr(*arg, cache, expr);
                    *arg = id;
                }
                expr.add(best_node)
            }
            let eclass_cost = ecost(eclass, &self.enodes_cache, &mut self.cost_cache);
            let eclass_cost = eclass_cost.1.clone();
            let mut expr = RecExpr::<SymbolLang>::default();
            build_rec_expr(eclass, &self.cost_cache, &mut expr);
            Some((eclass_cost, expr))
        }
    }
}

pub type VegCloning<G> = vegcloning::VersionedEGraph<G>;
pub type VegCloningBasic = VegCloning<crate::veg::structures::egraph::basic::EGraph<()>>;
pub type VegCloningPersistent = VegCloning<crate::veg::structures::egraph::persistent::EGraph<()>>;

pub mod vegcloning {
    use super::*;
    use crate::veg::structures::egraph::{EGraph as _, EGraphView as _};

    pub mod ext {
        pub use crate::veg::structures::egraph::ematching::machine;
        pub use crate::veg::structures::egraph::*;
        pub use crate::veg::util::id::*;
    }

    trait EmatchingCache {
        fn with_ematching_cache() -> Self;
    }
    impl<A: ext::Analysis> EmatchingCache for ext::basic::EGraph<A> {
        fn with_ematching_cache() -> Self {
            Self::with_ematching_cache()
        }
    }
    impl<A: ext::Analysis> EmatchingCache for ext::persistent::EGraph<A> {
        fn with_ematching_cache() -> Self {
            Self::with_ematching_cache()
        }
    }

    #[derive(Debug)]
    pub struct VersionedEGraph<
        G: ext::EGraph + ext::machine::MachineRequirements + Debug + Clone + EmatchingCache,
    > {
        branches: Vec<G>,
        checkout: Branch,
    }
    impl<G: ext::EGraph + ext::machine::MachineRequirements + Debug + Clone + EmatchingCache>
        VersionedEGraph<G>
    {
        fn proj(&self) -> &G {
            &self.branches[self.checkout.id]
        }
        fn proj_mut(&mut self) -> &mut G {
            &mut self.branches[self.checkout.id]
        }
    }
    impl<G: ext::EGraph + ext::machine::MachineRequirements + Debug + Clone + EmatchingCache>
        Default for VersionedEGraph<G>
    {
        fn default() -> Self {
            let mut egraph = VersionedEGraph {
                branches: vec![G::with_ematching_cache()],
                checkout: Branch { id: 0 },
            };
            egraph.checkout(egraph.checkout);
            egraph
        }
    }
    impl<G: ext::EGraph + ext::machine::MachineRequirements + Debug + Clone + EmatchingCache>
        EGraphView for VersionedEGraph<G>
    {
        fn classes(&self) -> impl Iterator<Item = Id> {
            // TODO Avoid cloning: for now cloning is required because of
            // borrow-checking problems with projections
            use ext::machine::MachineRequirements;
            self.proj()
                .canonicals()
                .map(|xc| veg::conversions::id.ic(&xc))
                .collect::<Vec<_>>()
                .into_iter()
        }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
            self.proj()
                .universe()
                .map(|xc| {
                    (
                        veg::conversions::id.ic(&xc),
                        veg::conversions::enode.ic(self.proj().get_enode(xc)),
                    )
                })
                .into_group_map()
        }
        fn total_number_of_nodes(&self) -> usize {
            self.proj()
                .universe()
                .map(|xc| {
                    (
                        veg::conversions::id.ic(&xc),
                        veg::conversions::enode.ic(self.proj().get_enode(xc)),
                    )
                })
                .count()
        }
        fn lookup(&self, x: &mut SymbolLang) -> Option<Id> {
            self.proj()
                .get_class(&veg::conversions::enode.c(x))
                .map(|xc| veg::conversions::id.ic(xc))
        }
        fn find(&self, x: Id) -> Id {
            veg::conversions::id.ic(&self.proj().find(veg::conversions::id.c(&x)))
        }

        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> {
            let searcher = veg::conversions::mpattern.c(searcher);
            let r = ext::machine::CanEMatch::ematch(&searcher, self.proj())
                .into_iter()
                .enumerate()
                .map(|(i, m)| {
                    let result = SearchMatches {
                        eclass: veg::conversions::id.ic(&m.eclass),
                        substs: m
                            .substs
                            .into_iter()
                            .map(|s| veg::conversions::msubst.ic(&s))
                            .collect(),
                    };
                    result
                })
                .collect();
            r
        }

        fn extractor(&self) -> impl Extractor<Self> {
            veg::VegExtractor::new(self)
        }

        fn branch(&self) -> Branch {
            self.checkout
        }
        fn branch_count(&self) -> usize {
            self.branches.len()
        }
    }
    impl<G: ext::EGraph + ext::machine::MachineRequirements + Debug + Clone + EmatchingCache> EGraph
        for VersionedEGraph<G>
    {
        fn add(&mut self, xn: SymbolLang) -> Id {
            let xc: ext::EClass = self.proj_mut().add(veg::conversions::enode.c(&xn));
            veg::conversions::id.ic(&xc)
        }
        fn add_expr(&mut self, recx: &RecExpr<SymbolLang>) -> Id {
            let mut cs: Vec<ext::EClass> = vec![];
            let mut proj = self.proj_mut();
            for xn in veg::conversions::recexpr::<SymbolLang>::new().c(recx) {
                cs.push(
                    proj.add(ext::ENode::application(
                        veg::conversions::op.c(&xn.op),
                        xn.children
                            .iter()
                            .map(|arg| cs[Into::<usize>::into(*arg)])
                            .collect(),
                    )),
                );
            }
            veg::conversions::id.ic(cs.last().unwrap())
        }
        fn union(&mut self, x: Id, y: Id) -> Id {
            veg::conversions::id.ic(&self
                .proj_mut()
                .union(veg::conversions::id.c(&x), veg::conversions::id.c(&y)))
        }
        fn rebuild(&mut self) {
            self.proj_mut().rebuild()
        }
        fn equivs(&mut self, recx: &RecExpr<SymbolLang>, recy: &RecExpr<SymbolLang>) -> Vec<Id> {
            fn lookup_expr<G: ext::EGraph>(
                egraph: &mut G,
                recx: &RecExpr<SymbolLang>,
            ) -> Option<ext::EClass> {
                let mut cs: Vec<ext::EClass> = vec![];
                for xn in veg::conversions::recexpr::<SymbolLang>::new().c(recx) {
                    let xc = egraph.get_class(&ext::ENode::application(
                        veg::conversions::op.c(&xn.op),
                        xn.children
                            .iter()
                            .map(|arg| cs[Into::<usize>::into(*arg)])
                            .collect(),
                    ));
                    if let Some(xc) = xc {
                        cs.push(egraph.find(*xc));
                    } else {
                        return None;
                    }
                }
                cs.last().cloned()
            }
            let mut proj = self.proj_mut();
            let xc = lookup_expr(proj, recx);
            let yc = lookup_expr(proj, recy);
            xc.filter(|xc| yc.is_some_and(|yc| *xc == yc))
                .map(|xc| vec![veg::conversions::id.ic(&xc)])
                .unwrap_or_default()
        }

        fn write_pattern(
            &mut self,
            applier: &Pattern<SymbolLang>,
            eclass: Id,
            subst: &Subst,
        ) -> Vec<Id> {
            let applier = veg::conversions::mpattern.c(applier);
            let eclass = veg::conversions::id.c(&eclass);
            let subst = veg::conversions::msubst.c(subst);
            let mut result = ext::machine::CanBind::bind(&applier, self.proj_mut(), eclass, &subst)
                .into_iter()
                .map(|c| veg::conversions::id.ic(&c))
                .collect::<Vec<_>>();
            if result.is_empty() {
                result.push(veg::conversions::id.ic(&eclass));
            }
            result
        }
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> {
            fn check_time(
                result: &mut Result<(), StopReason>,
                start_time: Instant,
                config: &RunnerConfig,
            ) {
                if let Some(timeout) = config.timeout {
                    let elapsed = start_time.elapsed();
                    if elapsed > timeout {
                        *result = Err(StopReason::TimeLimit(elapsed.as_secs_f64()));
                    }
                }
            }
            fn check_nodes<G: ext::EGraphView>(
                result: &mut Result<(), StopReason>,
                egraph: &G,
                config: &RunnerConfig,
            ) {
                if let Some(node_limit) = config.node_limit {
                    let size = egraph.universe().count();
                    if size > node_limit {
                        *result = Err(StopReason::NodeLimit(size));
                    }
                }
            }
            fn check_iters(
                result: &mut Result<(), StopReason>,
                iterations: usize,
                config: &RunnerConfig,
            ) {
                if let Some(iter_limit) = config.iter_limit {
                    if iterations > iter_limit {
                        *result = Err(StopReason::IterationLimit(iterations));
                    }
                }
            }
            let rules = rules.iter().collect::<Vec<_>>();
            let mut iterations: usize = 0;
            let mut result: Result<(), StopReason> = Ok(());
            let start_time: Instant = Instant::now();
            loop {
                self.proj_mut().rebuild();

                if result.is_ok() {
                    check_time(&mut result, start_time, config);
                    check_nodes(&mut result, self.proj(), config);
                    check_iters(&mut result, iterations, config);
                }
                if let Err(stop_reason) = result {
                    return Some(stop_reason);
                }

                let mut matches: Vec<Vec<SearchMatches>> = Vec::new();
                for rule in rules.iter() {
                    matches.push(rule.searcher.search(self));
                    check_time(&mut result, start_time, config);
                    if let Err(timeout) = result {
                        return Some(timeout);
                    }
                }

                let mut applications: HashMap<&String, usize> = HashMap::new();
                for (rule, matched) in rules.iter().zip(matches) {
                    let new_applications = rule.applier.apply_matches(self, &matched).len();
                    if new_applications > 0 {
                        *applications.entry(rule.name()).or_default() += new_applications;
                    }
                    check_time(&mut result, start_time, config);
                    if let Err(timeout) = result {
                        return Some(timeout);
                    }
                }

                if applications.is_empty() {
                    result = Err(StopReason::Saturated)
                }
                iterations += 1;
            }
        }

        fn branchout(&mut self) -> Branch {
            let current = &self.branches[self.checkout.id];
            self.branches.push(current.clone());
            Branch {
                id: self.branches.len() - 1,
            }
        }
        fn checkout(&mut self, branch: Branch) {
            self.checkout = branch
        }
    }
}
