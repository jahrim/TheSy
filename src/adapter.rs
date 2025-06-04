use std::{collections::{BTreeMap, HashMap}, time::Duration};
use egg::{Applier, CostFunction, Id, RecExpr, Rewrite, SearchMatches, Searcher, StopReason, Subst, SymbolLang};

use crate::eggstentions::costs::RepOrder;

pub type NoOp = ();
pub type Egg = egg::EGraph<SymbolLang, ()>;
pub type Vegg = ();
pub type EasterEgg = ();

#[derive(Default)]
pub struct RunnerConfig {
    pub timeout: Option<Duration>, 
    pub node_limit: Option<usize>, 
    pub iter_limit: Option<usize>
}

pub trait EGraph: Default + Clone + Sized {
    type Runner: Runner<Self>;
    fn add(&mut self, enode: SymbolLang) -> Id;
    fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id;
    fn lookup(&self, enode: &mut SymbolLang) -> Option<Id>;
    fn find(&self, eclass: Id) -> Id;
    fn union(&mut self, left: Id, right: Id) -> Id;
    fn rebuild(&mut self);
    fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id>;
    fn total_number_of_nodes(&self) -> usize;
    fn classes(&self) -> impl Iterator<Item = Id> + '_;
    fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>>;
    fn search(&self, searcher: &dyn Searcher<SymbolLang, ()>) -> Vec<SearchMatches>;
    fn apply(&mut self, applier: &dyn Applier<SymbolLang, ()>, eclass: Id, subst: &Subst) -> Vec<Id>;
    fn runner(self, config: &RunnerConfig) -> Self::Runner;
    fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C>;
}

impl EGraph for () {
    type Runner = ();
    fn add(&mut self, enode: SymbolLang) -> Id {
        Default::default()
    }
    fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id {
        Default::default()
    }
    fn classes(&self) -> impl Iterator<Item = Id> + '_ {
        std::iter::empty()
    }
    fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>>  {
        Default::default()
    }
    fn find(&self, eclass: Id) -> Id {
        Default::default()
    }
    fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> {
        None
    }
    fn rebuild(&mut self) {
        
    }
    fn total_number_of_nodes(&self) -> usize {
        Default::default()
    }
    fn union(&mut self, left: Id, right: Id) -> Id {
        Default::default()
    }
    fn search(&self, searcher: &dyn Searcher<SymbolLang, ()>) -> Vec<SearchMatches> {
        Default::default()
    }
    fn apply(&mut self, applier: &dyn Applier<SymbolLang, ()>, eclass: Id, subst: &Subst) -> Vec<Id> {
        Default::default()
    }
    fn runner(self, config: &RunnerConfig) -> Self::Runner {
    }
    fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> {
        Default::default()
    }
    fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C> {
        ()
    }
}

impl EGraph for Egg {
    type Runner = egg::Runner<SymbolLang, ()>;
    fn add(&mut self, enode: SymbolLang) -> Id {
        egg::EGraph::add(self, enode)
    }
    fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id {
        egg::EGraph::add_expr(self, expr)
    }
    fn classes(&self) -> impl Iterator<Item = Id> + '_ {
        egg::EGraph::classes(self).map(|xc| xc.id)
    }
    fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
        egg::EGraph::classes(self).map(|xc| (xc.id, xc.nodes.clone())).collect()
    }
    fn find(&self, eclass: Id) -> Id {
        egg::EGraph::find(self, eclass)
    }
    fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> {
        egg::EGraph::lookup(self, enode)
    }
    fn rebuild(&mut self) {
        egg::EGraph::rebuild(self);
    }
    fn search(&self, searcher: &dyn Searcher<SymbolLang, ()>) -> Vec<SearchMatches> {
        searcher.search(self)
    }
    fn total_number_of_nodes(&self) -> usize {
        egg::EGraph::total_number_of_nodes(self)
    }
    fn union(&mut self, left: Id, right: Id) -> Id {
        egg::EGraph::union(self, left, right).0
    }
    fn apply(&mut self, applier: &dyn Applier<SymbolLang, ()>, eclass: Id, subst: &Subst) -> Vec<Id> {
        applier.apply_one(self, eclass, subst)
    }
    fn runner(self, config: &RunnerConfig) -> Self::Runner {
        let mut runner = egg::Runner::default().with_egraph(self);
        if let Some(timeout) = config.timeout { runner = runner.with_time_limit(timeout) }
        if let Some(node_limit) = config.node_limit { runner = runner.with_node_limit(node_limit) }
        if let Some(iter_limit) = config.iter_limit { runner = runner.with_iter_limit(iter_limit) }
        runner
    }
    fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> {
        egg::EGraph::equivs(self, left, right)
    }
    fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C> {
        egg::Extractor::new(self, cost)
    }
}

pub trait Runner<G: EGraph> {
    fn egraph(self) -> G;
    fn run(self, rules: &[Rewrite<SymbolLang, ()>]) -> Self;
    fn stop_reason(&self) -> Option<StopReason>;
}

impl Runner<()> for () {
    fn egraph(self) -> () {
    }
    fn run(self, rules: &[Rewrite<SymbolLang, ()>]) -> Self {
    }
    fn stop_reason(&self) -> Option<StopReason> {
        Some(StopReason::Saturated)
    }
}

impl Runner<Egg> for egg::Runner<SymbolLang, ()> {
    fn egraph(self) -> Egg {
        self.egraph
    }
    
    fn run(self, rules: &[Rewrite<SymbolLang, ()>]) -> Self {
        egg::Runner::run(self, rules)
    }
    fn stop_reason(&self) -> Option<StopReason> {
        self.stop_reason.clone()
    }
}

pub trait Extractor<G: EGraph, C: CostFunction<SymbolLang>> {
    fn find_best(&mut self, eclass: Id) -> Option<(C::Cost, RecExpr<SymbolLang>)>;
}


impl<C: CostFunction<SymbolLang>> Extractor<(), C> for () {
    fn find_best(&mut self, eclass: Id) -> Option<(<C as CostFunction<SymbolLang>>::Cost, RecExpr<SymbolLang>)> {
        Default::default()
    }
}

impl<'a, C: CostFunction<SymbolLang>> Extractor<Egg, C> for egg::Extractor<'a, C, SymbolLang, ()> {
    fn find_best(&mut self, eclass: Id) -> Option<(<C as CostFunction<SymbolLang>>::Cost, RecExpr<SymbolLang>)> {
        Some(egg::Extractor::find_best(self, eclass))
    }
}