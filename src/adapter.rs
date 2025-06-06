use std::{collections::HashMap, time::Duration};
use crate::egg::{Pattern, CostFunction, Id, RecExpr, SearchMatches, StopReason, Subst, SymbolLang};
use crate::eggstentions::{rewrites::Rewrite, appliers::{Applier, HasApplyOne}, searchers::multisearcher::{HasSearch, Searcher}};

pub use common::*;

pub mod common {

    use super::*;

    #[derive(Default)]
    pub struct RunnerConfig {
        pub timeout: Option<Duration>, 
        pub node_limit: Option<usize>, 
        pub iter_limit: Option<usize>
    }

    #[derive(Clone, Copy, Default)]
    pub struct Branch { pub id: usize }

    pub trait Searchable { 
        fn search(&self, searcher: &Searcher) -> Vec<SearchMatches>;
    }
    pub trait Writable {
        fn write(&mut self, applier: &Applier, eclass: Id, subst: &Subst) -> Vec<Id>;
    }

    pub trait EGraph: Default + Sized {
        fn classes(&self) -> impl Iterator<Item = Id> + '_;
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>>;
        fn total_number_of_nodes(&self) -> usize;
        fn lookup(&self, enode: &mut SymbolLang) -> Option<Id>;
        fn find(&self, eclass: Id) -> Id;

        fn add(&mut self, enode: SymbolLang) -> Id;
        fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id;
        fn union(&mut self, left: Id, right: Id) -> Id;
        fn rebuild(&mut self);
        fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id>;

        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches>;
        fn write_pattern(&mut self, applier: &Pattern<SymbolLang>, eclass: Id, subst: &Subst) -> Vec<Id>;
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason>;

        fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C>;

        fn branch(&self) -> Branch;
        fn branchout(&mut self) -> Branch;
        fn checkout(&mut self, branch: Branch);
    }

    impl<G: EGraph> Searchable for G {
        fn search(&self, searcher: &Searcher) -> Vec<SearchMatches> { searcher.search(self) }
    }
    impl<G: EGraph> Writable for G {
        fn write(&mut self, applier: &Applier, eclass: Id, subst: &Subst) -> Vec<Id> { applier.apply_one(self, eclass, subst) }
    }

    pub trait Extractor<G: EGraph, C: CostFunction<SymbolLang>> {
        fn find_best(&mut self, eclass: Id) -> Option<(C::Cost, RecExpr<SymbolLang>)>;
    }

    pub trait Conversion {
        type Left;
        type Right;
        fn conversion(source: &Self::Left) -> Self::Right;
        fn inverse_conversion(source: &Self::Right) -> Self::Left;
        fn c(source: &Self::Left) -> Self::Right { Self::conversion(source) }
        fn ic(source: &Self::Right) -> Self::Left { Self::inverse_conversion(source) }
    }
}

pub type NoOp = ();
pub mod noop {
    use super::*;

    impl EGraph for () {
        fn classes(&self) -> impl Iterator<Item = Id> + '_ { std::iter::empty() }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> { Default::default() }
        fn total_number_of_nodes(&self) -> usize { Default::default() }
        fn lookup(&self, _: &mut SymbolLang) -> Option<Id> { None }
        fn find(&self, _: Id) -> Id { Default::default() }

        fn add(&mut self, _: SymbolLang) -> Id { Default::default() }
        fn add_expr(&mut self, _: &RecExpr<SymbolLang>) -> Id { Default::default() }
        fn union(&mut self, _: Id, _: Id) -> Id { Default::default() }
        fn rebuild(&mut self) { }
        fn equivs(&mut self, _: &RecExpr<SymbolLang>, _: &RecExpr<SymbolLang>) -> Vec<Id> { Default::default() }

        fn search_pattern(&self, _: &Pattern<SymbolLang>) -> Vec<SearchMatches> { Default::default() }
        fn write_pattern(&mut self, applier: &Pattern<SymbolLang>, eclass: Id, subst: &Subst) -> Vec<Id> { Default::default() }
        fn run(&mut self, _: &RunnerConfig, _: &[Rewrite]) -> Option<StopReason> { Some(StopReason::Saturated) }

        fn extractor<C: CostFunction<SymbolLang>>(&self, _: C) -> impl Extractor<Self, C> { }

        fn checkout(&mut self, _: Branch) { }
        fn branchout(&mut self) -> Branch { Default::default() }
        fn branch(&self) -> Branch { Default::default() }
    }

    impl<C: CostFunction<SymbolLang>> Extractor<(), C> for () {
        fn find_best(&mut self, _: Id) -> Option<(<C as CostFunction<SymbolLang>>::Cost, RecExpr<SymbolLang>)> { Default::default() }
    }
}

pub type Egg = egg::VersionedEGraph;
pub mod egg {
    use super::*;
    use crate::egg;

    pub mod conversions {
        use super::*;

        #[allow(non_camel_case_types)] pub struct rw;
        impl Conversion for rw {
            type Left = egg::Rewrite<egg::SymbolLang, ()>;
            type Right = Rewrite;
            fn conversion(_: &Self::Left) -> Self::Right {
                unimplemented!("cannot convert dyn searcher and appliers")
            }
            fn inverse_conversion(source: &Self::Right) -> Self::Left {
                egg::Rewrite::new(source.name(), source.searcher.clone(), source.applier.clone()).unwrap()
            }
        }
    }
    impl EGraph for egg::EGraph<SymbolLang, ()> {
        fn classes(&self) -> impl Iterator<Item = Id> + '_ { egg::EGraph::classes(self).map(|xc| xc.id) }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> { egg::EGraph::classes(self).map(|xc| (xc.id, xc.nodes.clone())).collect() }
        fn total_number_of_nodes(&self) -> usize { egg::EGraph::total_number_of_nodes(self) }
        fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> { egg::EGraph::lookup(self, enode) }
        fn find(&self, eclass: Id) -> Id { egg::EGraph::find(self, eclass) }

        fn add(&mut self, enode: SymbolLang) -> Id { egg::EGraph::add(self, enode) }
        fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id { egg::EGraph::add_expr(self, expr) }
        fn union(&mut self, left: Id, right: Id) -> Id { egg::EGraph::union(self, left, right).0 }
        fn rebuild(&mut self) { egg::EGraph::rebuild(self); }
        fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> { egg::EGraph::equivs(self, left, right) }
        
        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> { egg::Searcher::search(searcher, self) }
        fn write_pattern(&mut self, applier: &Pattern<SymbolLang>, eclass: Id, subst: &Subst) -> Vec<Id> { egg::Applier::apply_one(applier, self, eclass, subst) }
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> {
            let mut runner = egg::Runner::default().with_egraph(std::mem::take(self));
            if let Some(timeout) = config.timeout { runner = runner.with_time_limit(timeout) }
            if let Some(node_limit) = config.node_limit { runner = runner.with_node_limit(node_limit) }
            if let Some(iter_limit) = config.iter_limit { runner = runner.with_iter_limit(iter_limit) }
            runner = runner.run(&rules.iter().map(conversions::rw::ic).collect::<Vec<_>>());
            runner.egraph.rebuild();
            *self = runner.egraph;
            runner.stop_reason
        }

        fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C> { egg::Extractor::new(self, cost) }
        
        fn branch(&self) -> Branch { unimplemented!("cannot branch from traditional egraphs") }
        fn branchout(&mut self) -> Branch { unimplemented!("cannot branch from traditional egraphs") }
        fn checkout(&mut self, branch: Branch) { unimplemented!("cannot branch from traditional egraphs") }
    }
    impl<C: CostFunction<SymbolLang>> Extractor<egg::EGraph<SymbolLang, ()>, C> for egg::Extractor<'_, C, SymbolLang, ()> {
        fn find_best(&mut self, eclass: Id) -> Option<(<C as CostFunction<SymbolLang>>::Cost, RecExpr<SymbolLang>)> {
            Some(egg::Extractor::find_best(self, eclass))
        }
    }

    pub struct VersionedEGraph {
        branches: Vec<egg::EGraph<SymbolLang, ()>>,
        checkout: Branch,
    }
    impl Default for VersionedEGraph {
        fn default() -> Self {
            VersionedEGraph { branches: vec![Default::default()], checkout: Branch { id: 0 } }
        }
    }
    impl EGraph for VersionedEGraph {
        fn classes(&self) -> impl Iterator<Item = Id> + '_ { EGraph::classes(&self.branches[self.checkout.id]) }
        fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> { EGraph::enodes(&self.branches[self.checkout.id]) }
        fn total_number_of_nodes(&self) -> usize { EGraph::total_number_of_nodes(&self.branches[self.checkout.id]) }
        fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> { EGraph::lookup(&self.branches[self.checkout.id], enode) }
        fn find(&self, eclass: Id) -> Id { EGraph::find(&self.branches[self.checkout.id], eclass) }

        fn add(&mut self, enode: SymbolLang) -> Id { EGraph::add(&mut self.branches[self.checkout.id], enode) }
        fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id { EGraph::add_expr(&mut self.branches[self.checkout.id], expr) }
        fn union(&mut self, left: Id, right: Id) -> Id { EGraph::union(&mut self.branches[self.checkout.id], left, right) }
        fn rebuild(&mut self) { EGraph::rebuild(&mut self.branches[self.checkout.id]); }
        fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> { EGraph::equivs(&mut self.branches[self.checkout.id], left, right) }
        
        fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> { EGraph::search_pattern(&self.branches[self.checkout.id], searcher) }
        fn write_pattern(&mut self, applier: &Pattern<SymbolLang>, eclass: Id, subst: &Subst) -> Vec<Id> { EGraph::write_pattern(&mut self.branches[self.checkout.id], applier, eclass, subst) }
        fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite]) -> Option<StopReason> { EGraph::run(&mut self.branches[self.checkout.id], config, rules) }

        fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C> { egg::Extractor::new(&self.branches[self.checkout.id], cost) }
        
        fn branch(&self) -> Branch { self.checkout }
        fn branchout(&mut self) -> Branch {
            let current = &self.branches[self.checkout.id];
            self.branches.push(current.clone());
            Branch { id: self.branches.len() - 1 }
        }
        fn checkout(&mut self, branch: Branch) { self.checkout = branch }
    }

    impl<C: CostFunction<SymbolLang>> Extractor<Egg, C> for egg::Extractor<'_, C, SymbolLang, ()> {
        fn find_best(&mut self, eclass: Id) -> Option<(<C as CostFunction<SymbolLang>>::Cost, RecExpr<SymbolLang>)> {
            Some(egg::Extractor::find_best(self, eclass))
        }
    }
}

pub type EasterEgg = ();
pub mod easteregg {
    // use std::any;

    // use super::*;
    // use easter_egg;

    // pub struct VersionedEGraph {
    //     egraph: easter_egg::EGraph<easter_egg::SymbolLang, ()>,
    //     branches: Vec<easter_egg::ColorId>,
    //     checkout: Branch,
    // }
    // impl Default for VersionedEGraph {
    //     fn default() -> Self {
    //         let mut easteregg = VersionedEGraph { 
    //             egraph: Default::default(),
    //             branches: vec![], 
    //             checkout: Branch { id: 0 } 
    //         };
    //         let root = easteregg.egraph.create_color(None);
    //         easteregg.branches.push(root);
    //         easteregg
    //     }
    // }
    // pub mod conversions {
    //     use super::*;

    //     #[allow(non_camel_case_types)] pub struct id;
    //     impl Conversion for id {
    //         type Left = easter_egg::Id;
    //         type Right = Id;
    //         fn conversion(source: &Self::Left) -> Self::Right {
    //             let i: usize = source.0 as usize;
    //             Id::from(i) 
    //         }
    //         fn inverse_conversion(source: &Self::Right) -> Self::Left {
    //             let i: usize = (*source).into();
    //             easter_egg::Id::from(i)
    //         }
    //     }
    //     #[allow(non_camel_case_types)] pub struct enode;
    //     impl Conversion for enode {
    //         type Left = easter_egg::SymbolLang;
    //         type Right = SymbolLang;
    //         fn conversion(source: &Self::Left) -> Self::Right {
    //             SymbolLang {
    //                 op: crate::egg::Symbol::from(source.op.as_str()),
    //                 children: source.children.iter().map(id::c).collect()
    //             }
    //         }
    //         fn inverse_conversion(source: &Self::Right) -> Self::Left {
    //             easter_egg::SymbolLang { 
    //                 op: easter_egg::Symbol::from(source.op.as_str()), 
    //                 children: source.children.iter().map(id::ic).collect()
    //             }
    //         }
    //     }
    //     #[allow(non_camel_case_types)] pub struct recexpr;
    //     impl Conversion for recexpr {
    //         type Left = easter_egg::RecExpr<easter_egg::SymbolLang>;
    //         type Right = RecExpr<SymbolLang>;
    //         fn conversion(source: &Self::Left) -> Self::Right {
    //             source.to_string().as_str().parse().unwrap()
    //         }
    //         fn inverse_conversion(source: &Self::Right) -> Self::Left {
    //             source.to_string().as_str().parse().unwrap()
    //         }
    //     }
    // }
    // impl EGraph for VersionedEGraph {
    //     fn classes(&self) -> impl Iterator<Item = Id> + '_ { 
    //         let color = self.branches[self.checkout.id];
    //         self.egraph
    //             .classes()
    //             .filter(move |xc| xc.color().is_some_and(|c| c == color))
    //             .map(|xc| conversions::id::c(&xc.id))
    //     }
    //     fn enodes(&self) -> HashMap<Id, Vec<SymbolLang>> {
    //         let color = self.branches[self.checkout.id];
    //         self.egraph
    //             .classes()
    //             .filter(move |xc| xc.color().is_some_and(|c| c == color))
    //             .map(|xc| (conversions::id::c(&xc.id), xc.nodes.iter().map(conversions::enode::c).collect::<Vec<_>>()))
    //             .collect() 
    //     }
    //     fn total_number_of_nodes(&self) -> usize { 
    //         // TODO this has better performances but is overestimating the number of enodes in the
    //         //      current branch
    //         self.egraph.total_number_of_nodes()
    //     }
    //     fn lookup(&self, enode: &mut SymbolLang) -> Option<Id> { 
    //         let color = self.branches[self.checkout.id];
    //         self.egraph.colored_lookup(color, conversions::enode::ic(enode)).map(|id| conversions::id::c(&id))
    //     }
    //     fn find(&self, eclass: Id) -> Id { 
    //         let color = self.branches[self.checkout.id];
    //         conversions::id::c(&self.egraph.colored_find(color, conversions::id::ic(&eclass)))
    //     }

    //     fn add(&mut self, enode: SymbolLang) -> Id { 
    //         let color = self.branches[self.checkout.id];
    //         conversions::id::c(&self.egraph.colored_add(color, conversions::enode::ic(&enode)))
    //     }
    //     fn add_expr(&mut self, expr: &RecExpr<SymbolLang>) -> Id {
    //         let color = self.branches[self.checkout.id];
    //         conversions::id::c(&self.egraph.colored_add_expr(color, &conversions::recexpr::ic(&expr)))
    //     }
    //     fn union(&mut self, left: Id, right: Id) -> Id {
    //         let color = self.branches[self.checkout.id];
    //         conversions::id::c(&self.egraph.colored_union(color, conversions::id::ic(&left), conversions::id::ic(&right)).0)
    //     }
    //     fn rebuild(&mut self) { 
    //         self.egraph.rebuild();
    //     }
    //     fn equivs(&mut self, left: &RecExpr<SymbolLang>, right: &RecExpr<SymbolLang>) -> Vec<Id> { 
    //         self.egraph
    //             .equivs(&conversions::recexpr::ic(left), &conversions::recexpr::ic(right))
    //             .into_iter()
    //             .map(|xc| conversions::id::c(&xc))
    //             .collect()
    //      }
        
    //     fn search_pattern(&self, searcher: &Pattern<SymbolLang>) -> Vec<SearchMatches> { 
    //         todo!() 
    //     }
    //     fn apply(&mut self, applier: &dyn Applier<SymbolLang, ()>, eclass: Id, subst: &Subst) -> Vec<Id> { todo!() }
    //     fn run(&mut self, config: &RunnerConfig, rules: &[Rewrite<SymbolLang, ()>]) -> Option<StopReason> { todo!() }

    //     fn extractor<C: CostFunction<SymbolLang>>(&self, cost: C) -> impl Extractor<Self, C> { todo!() }
        
    //     fn branch(&self) -> Branch { self.checkout }
    //     fn branchout(&mut self) -> Branch { 
    //         let color = self.branches[self.checkout.id];
    //         let new_color = self.egraph.create_color(Some(color));
    //         self.branches.push(new_color);
    //         Branch { id: self.branches.len() - 1  }
    //     }
    //     fn checkout(&mut self, branch: Branch) { 
    //         self.checkout = branch
    //      }
    // }

    // impl<C: CostFunction<SymbolLang>> Extractor<Egg, C> for easter_egg::Extractor<'_, C, SymbolLang, ()> {
    //     fn find_best(&mut self, eclass: Id) -> Option<(<C as CostFunction<SymbolLang>>::Cost, RecExpr<SymbolLang>)> {
    //         todo!()
    //     }
    // }
}

pub type Vegg = ();
pub mod vegg {
    use super::*;
}