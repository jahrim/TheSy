use std::fmt::Display;

use crate::eggstentions::pretty_string::PrettyString;
use egg::{Id, SearchMatches, Subst, SymbolLang, Var};
use itertools::Itertools;

use crate::{adapter::EGraph, eggstentions::searchers::multisearcher::HasVars};

pub trait AsApplier {
    fn as_applier(self) -> Applier;
}
pub trait HasApplyOne {
    fn apply_one<G: EGraph>(&self, egraph: &mut G, eclass: Id, subst: &Subst) -> Vec<Id>;
}
pub trait HasApplyMatches {
    fn apply_matches<G: EGraph>(&self, egraph: &mut G, matches: &[SearchMatches]) -> Vec<Id>;
}

macro_rules! Applier {
    ({ $($variant:ident($type:ty)),* $(,)? }) => {
        #[derive(Clone, Debug)]
        pub enum Applier { $($variant($type)),* }
        impl AsApplier for Applier {
            #[inline(always)] fn as_applier(self) -> Applier { self }
        }
        impl HasApplyOne for Applier {
            fn apply_one<G: EGraph>(&self, egraph: &mut G, eclass: Id, subst: &Subst) -> Vec<Id> {
                match self { $(Self::$variant(applier) => applier.apply_one(egraph, eclass, subst)),* }
            }
        }
        impl HasApplyMatches for Applier {
            fn apply_matches<G: EGraph>(&self, egraph: &mut G, matches: &[SearchMatches]) -> Vec<Id> {
                match self { $(Self::$variant(applier) => applier.apply_matches(egraph, matches)),* }
            }
        }
        impl HasVars for Applier {
            fn vars(&self) -> Vec<Var> {
                match self { $(Self::$variant(applier) => applier.vars()),* }
            }
        }
        impl PrettyString for Applier {
            fn pretty_string(&self) -> String {
                match self { $(Self::$variant(applier) => applier.pretty_string()),* }
            }
        }
    };
}
Applier!({
    Pattern(egg::Pattern<SymbolLang>),
    Diff(diff::DiffApplier),
    Union(union::UnionApplier),
});
impl Display for Applier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub mod pattern {
    use super::*;

    impl AsApplier for &str {
        fn as_applier(self) -> Applier {
            self.parse::<egg::Pattern<SymbolLang>>()
                .unwrap()
                .as_applier()
        }
    }
    impl AsApplier for String {
        fn as_applier(self) -> Applier {
            self.parse::<egg::Pattern<SymbolLang>>()
                .unwrap()
                .as_applier()
        }
    }
    impl AsApplier for egg::Pattern<SymbolLang> {
        fn as_applier(self) -> Applier {
            Applier::Pattern(self)
        }
    }
    impl HasApplyMatches for egg::Pattern<SymbolLang> {
        fn apply_matches<G: EGraph>(&self, egraph: &mut G, matches: &[SearchMatches]) -> Vec<Id> {
            let mut added = vec![];
            for mat in matches {
                for subst in &mat.substs {
                    let ids = self
                        .apply_one(egraph, mat.eclass, subst)
                        .into_iter()
                        .filter_map(|id| {
                            if id != mat.eclass {
                                Some(egraph.union(id, mat.eclass))
                            } else {
                                None
                            }
                        });
                    added.extend(ids)
                }
            }
            added
        }
    }
    impl HasApplyOne for egg::Pattern<SymbolLang> {
        fn apply_one<G: EGraph>(&self, egraph: &mut G, eclass: Id, subst: &Subst) -> Vec<Id> {
            egraph.write_pattern(self, eclass, subst)
        }
    }
}

pub mod diff {
    use std::sync::Arc;

    use super::*;

    #[derive(Clone, Debug)]
    pub struct DiffApplier {
        applier: Arc<Applier>,
    }
    impl DiffApplier {
        pub fn new(applier: impl AsApplier) -> DiffApplier {
            DiffApplier {
                applier: Arc::new(applier.as_applier()),
            }
        }
    }
    impl AsApplier for DiffApplier {
        fn as_applier(self) -> Applier {
            Applier::Diff(self)
        }
    }
    impl HasApplyMatches for DiffApplier {
        fn apply_matches<G: EGraph>(&self, egraph: &mut G, matches: &[SearchMatches]) -> Vec<Id> {
            let added = vec![];
            for mat in matches {
                for subst in &mat.substs {
                    let ids = self.apply_one(egraph, mat.eclass, subst);
                    //     .into_iter()
                    //     .filter_map(|id| {
                    //         let (to, did_something) = egraph.union(id, mat.eclass);
                    //         if did_something {
                    //             Some(to)
                    //         } else {
                    //             None
                    //         }
                    //     });
                    // added.extend(ids)
                }
            }
            added
        }
    }
    impl HasApplyOne for DiffApplier {
        fn apply_one<G: EGraph>(&self, egraph: &mut G, eclass: Id, subst: &Subst) -> Vec<Id> {
            self.applier.apply_one(egraph, eclass, subst)
        }
    }
    impl HasVars for DiffApplier {
        fn vars(&self) -> Vec<Var> {
            self.applier.vars()
        }
    }
    impl PrettyString for DiffApplier {
        fn pretty_string(&self) -> String {
            self.applier.pretty_string()
        }
    }
}
pub mod union {
    use super::*;

    #[derive(Clone, Debug)]
    pub struct UnionApplier {
        vars: Vec<Var>,
    }
    impl AsApplier for UnionApplier {
        fn as_applier(self) -> Applier {
            Applier::Union(self)
        }
    }
    impl UnionApplier {
        pub fn new(vars: Vec<Var>) -> UnionApplier {
            UnionApplier { vars }
        }
    }
    impl HasApplyOne for UnionApplier {
        fn apply_one<G: EGraph>(&self, egraph: &mut G, eclass: Id, subst: &Subst) -> Vec<Id> {
            unimplemented!()
        }
    }
    impl HasApplyMatches for UnionApplier {
        fn apply_matches<G: EGraph>(&self, egraph: &mut G, matches: &[SearchMatches]) -> Vec<Id> {
            let mut added = vec![];
            for mat in matches {
                for subst in &mat.substs {
                    let first = self.vars.first().unwrap();
                    let ids = self
                        .vars
                        .iter()
                        .skip(1)
                        .filter_map(|v| {
                            let x = *subst.get(*first).unwrap();
                            let y = *subst.get(*v).unwrap();
                            if x != y {
                                Some(egraph.union(x, y))
                            } else {
                                None
                            }
                        })
                        .collect_vec();
                    added.extend(ids)
                }
            }
            added
        }
    }
    impl HasVars for UnionApplier {
        fn vars(&self) -> Vec<Var> {
            self.vars.clone()
        }
    }
    impl PrettyString for UnionApplier {
        fn pretty_string(&self) -> String {
            format!(
                "Union({})",
                self.vars.iter().map(|x| x.to_string()).join(" ")
            )
        }
    }
}
