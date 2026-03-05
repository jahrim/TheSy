pub mod multisearcher {
    use std::collections::{HashMap, HashSet};
    use std::iter::FromIterator;
    use std::ops::Deref;
    use std::str::FromStr;

    use egg::{Id, Language, SearchMatches, Subst, SymbolLang, Var};
    use itertools::{Either, Itertools};

    use crate::adapter::{EGraph, EGraphView};
    use crate::eggstentions::pretty_string::PrettyString;
    use crate::tools::tools::Grouped;
    use smallvec::alloc::fmt::Formatter;
    use std::fmt::{Debug, Display};

    pub trait AsSearcher {
        fn as_searcher(self) -> Searcher;
    }
    pub trait HasSearchEClass {
        fn search_eclass<G: EGraphView>(&self, egraph: &G, eclass: Id) -> Option<SearchMatches>;
    }
    pub trait HasSearch {
        fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches>;
    }
    pub trait HasVars {
        fn vars(&self) -> Vec<Var>;
    }

    macro_rules! Searcher {
        ({ $($variant:ident($type:ty)),* $(,)? }) => {
            #[derive(Clone, Debug)]
            pub enum Searcher { $($variant($type)),* }
            impl AsSearcher for Searcher {
                #[inline(always)] fn as_searcher(self) -> Searcher { self }
            }
            impl HasSearchEClass for Searcher {
                fn search_eclass<G: EGraphView>(&self, egraph: &G, eclass: Id) -> Option<SearchMatches> {
                    match self { $(Self::$variant(searcher) => searcher.search_eclass(egraph, eclass)),* }
                }
            }
            impl HasSearch for Searcher {
                fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                    match self { $(Self::$variant(searcher) => searcher.search(egraph)),* }
                }
            }
            impl HasVars for Searcher {
                fn vars(&self) -> Vec<Var> {
                    match self { $(Self::$variant(searcher) => searcher.vars()),* }
                }
            }
            impl PrettyString for Searcher {
                fn pretty_string(&self) -> String {
                    match self { $(Self::$variant(searcher) => searcher.pretty_string()),* }
                }
            }
        };
    }

    Searcher!({
        Pattern(egg::Pattern<SymbolLang>),
        Either(either::EitherSearcher),
        MultiEq(multieq::MultiEqSearcher),
        MultiDiff(multidiff::MultiDiffSearcher),
        Filter(filter::FilterSearcher),
        Pointer(pointer::PointerSearcher),
    });
    impl Display for Searcher {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    impl Searcher {
        pub fn get_common_vars(patterns: &mut Vec<Searcher>) -> HashMap<Var, usize> {
            fn count_commons(p: &Searcher, common_vars: &HashMap<Var, usize>) -> usize {
                p.vars()
                    .iter()
                    .map(|v| common_vars.get(v).unwrap_or(&0))
                    .sum()
            }
            let common_vars = patterns
                .iter()
                .flat_map(|p| p.vars())
                .grouped(|v| v.clone())
                .iter()
                .filter_map(|(k, v)| {
                    if v.len() <= 1 {
                        None
                    } else {
                        Some((*k, v.len()))
                    }
                })
                .collect::<HashMap<Var, usize>>();
            patterns.sort_by_key(|p| count_commons(p, &common_vars));
            common_vars
        }
        pub fn merge_substs(vars: &Vec<Var>, sub1: &Subst, sub2: &Subst) -> Subst {
            let mut res = Subst::with_capacity(vars.len());
            for v in vars {
                let v1 = v.clone();
                let s1 = sub1.get(v1);
                let s2 = sub2.get(v1);
                if s1.is_some() || s2.is_some() {
                    if s1.is_some() && s2.is_some() {
                        assert_eq!(s1.as_ref(), s2.as_ref());
                    }
                    res.insert(v1, s1.unwrap_or_else(|| s2.unwrap()).clone());
                }
            }
            res
        }
        // Aggregate product of equal common var substs
        pub fn aggregate_substs(
            possibilities: &[HashMap<Vec<Option<Id>>, Vec<Subst>>],
            limits: Vec<&Option<Id>>,
            all_vars: &Vec<Var>,
        ) -> Vec<Subst> {
            let current = possibilities.first().unwrap();
            // TODO: if matches can be taken directly from limitations then do so
            let matches = current.iter().filter(|(keys, _)| {
                limits.iter().zip(keys.iter()).all(|(lim, key)| {
                    lim.as_ref()
                        .map_or(true, |l| key.as_ref().map_or(true, |k| k == l))
                })
            });
            if possibilities.len() > 1 {
                let mut collected = Vec::new();
                for (key, val) in matches {
                    let new_limit: Vec<&Option<Id>> = limits
                        .iter()
                        .zip(key)
                        .map(|(l, k)| if l.is_some() { l } else { k })
                        .collect();
                    let rec_res =
                        Searcher::aggregate_substs(&possibilities[1..], new_limit, all_vars);
                    collected.extend(
                        rec_res
                            .iter()
                            .cartesian_product(val)
                            .map(|(s1, s2)| Searcher::merge_substs(all_vars, s1, s2)),
                    );
                }
                collected
            } else {
                matches
                    .flat_map(|(_, v)| v.iter().map(|s| Searcher::merge_substs(all_vars, s, s)))
                    .collect()
            }
        }
        pub fn group_by_common_vars(
            mut search_results: Vec<&mut SearchMatches>,
            common_vars: &HashMap<Var, usize>,
        ) -> Vec<HashMap<Vec<Option<Id>>, Vec<Subst>>> {
            let mut by_vars: Vec<HashMap<Vec<Option<Id>>, Vec<Subst>>> = Vec::new();
            for matches in search_results.iter_mut() {
                let cur_map: HashMap<Vec<Option<Id>>, Vec<Subst>> = {
                    let substs: Vec<Subst> = std::mem::replace(&mut matches.substs, Vec::new());
                    let grouped = substs.into_iter().grouped(|s| {
                        common_vars
                            .keys()
                            .map(|v| s.get(v.clone()).map(|i| i.clone()))
                            .collect::<Vec<Option<Id>>>()
                    });
                    grouped
                };
                by_vars.push(cur_map);
            }
            by_vars
        }
    }
    pub mod pattern {
        use super::*;
        impl AsSearcher for &str {
            fn as_searcher(self) -> Searcher {
                self.parse::<egg::Pattern<SymbolLang>>()
                    .unwrap()
                    .as_searcher()
            }
        }
        impl AsSearcher for String {
            fn as_searcher(self) -> Searcher {
                self.parse::<egg::Pattern<SymbolLang>>()
                    .unwrap()
                    .as_searcher()
            }
        }
        impl AsSearcher for egg::Pattern<SymbolLang> {
            fn as_searcher(self) -> Searcher {
                Searcher::Pattern(self)
            }
        }
        impl HasSearchEClass for egg::Pattern<SymbolLang> {
            fn search_eclass<G: EGraphView>(
                &self,
                egraph: &G,
                eclass: Id,
            ) -> Option<SearchMatches> {
                unimplemented!()
            }
        }
        impl HasSearch for egg::Pattern<SymbolLang> {
            fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                egraph.search_pattern(self)
            }
        }
        impl HasVars for egg::Pattern<SymbolLang> {
            fn vars(&self) -> Vec<Var> {
                egg::Pattern::vars(self)
            }
        }
    }
    pub mod either {
        use std::sync::Arc;

        use super::*;
        pub struct EitherSearcher {
            node: Arc<Either<Searcher, Searcher>>,
        }

        impl EitherSearcher {
            pub fn left(left: impl AsSearcher) -> Self {
                EitherSearcher {
                    node: Arc::new(Either::Left(left.as_searcher())),
                }
            }
            pub fn right(right: impl AsSearcher) -> Self {
                EitherSearcher {
                    node: Arc::new(Either::Right(right.as_searcher())),
                }
            }
        }
        impl AsSearcher for EitherSearcher {
            fn as_searcher(self) -> Searcher {
                Searcher::Either(self)
            }
        }
        impl HasSearchEClass for EitherSearcher {
            fn search_eclass<G: EGraphView>(
                &self,
                egraph: &G,
                eclass: Id,
            ) -> Option<SearchMatches> {
                match self.node.deref() {
                    Either::Left(searcher) => searcher.search_eclass(egraph, eclass),
                    Either::Right(searcher) => searcher.search_eclass(egraph, eclass),
                }
            }
        }
        impl HasSearch for EitherSearcher {
            fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                match self.node.deref() {
                    Either::Left(searcher) => searcher.search(egraph),
                    Either::Right(searcher) => searcher.search(egraph),
                }
            }
        }
        impl HasVars for EitherSearcher {
            fn vars(&self) -> Vec<Var> {
                match self.node.deref() {
                    Either::Left(searcher) => searcher.vars(),
                    Either::Right(searcher) => searcher.vars(),
                }
            }
        }
        impl Clone for EitherSearcher {
            fn clone(&self) -> Self {
                match self.node.deref() {
                    Either::Left(searcher) => Self::left(searcher.clone()),
                    Either::Right(searcher) => Self::right(searcher.clone()),
                }
            }
        }
        impl Debug for EitherSearcher {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(&self.node, f)
            }
        }
        impl PrettyString for EitherSearcher {
            fn pretty_string(&self) -> String {
                match self.node.deref() {
                    Either::Left(searcher) => searcher.pretty_string(),
                    Either::Right(searcher) => searcher.pretty_string(),
                }
            }
        }
    }
    pub mod multieq {
        use super::*;
        pub struct MultiEqSearcher {
            patterns: Vec<Searcher>,
            common_vars: HashMap<Var, usize>,
        }
        impl MultiEqSearcher {
            pub(crate) fn new(patterns: Vec<impl AsSearcher>) -> MultiEqSearcher {
                let mut patterns: Vec<Searcher> =
                    patterns.into_iter().map(|p| p.as_searcher()).collect();
                let common_vars = Searcher::get_common_vars(&mut patterns);
                MultiEqSearcher {
                    patterns,
                    common_vars,
                }
            }
        }
        impl FromStr for MultiEqSearcher {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let patterns = s
                    .split("|||")
                    .map(|p| p.as_searcher())
                    .collect::<Vec<Searcher>>();
                if patterns.len() == 1 {
                    Err(String::from("Need at least two patterns"))
                } else {
                    Ok(MultiEqSearcher::new(patterns))
                }
            }
        }
        impl AsSearcher for MultiEqSearcher {
            fn as_searcher(self) -> Searcher {
                Searcher::MultiEq(self)
            }
        }
        impl HasSearchEClass for MultiEqSearcher {
            fn search_eclass<G: EGraphView>(&self, _: &G, _: Id) -> Option<SearchMatches> {
                unimplemented!()
            }
        }
        impl HasSearch for MultiEqSearcher {
            fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                if self.patterns.len() == 1 {
                    return self.patterns[0].search(egraph);
                }

                let mut search_results = {
                    let mut res = Vec::new();
                    for p in self.patterns.iter() {
                        let mut current = HashMap::new();
                        let searched = p.search(egraph);
                        for s in searched {
                            assert!(!current.contains_key(&s.eclass));
                            current.insert(s.eclass, s);
                        }
                        res.push(current);
                    }
                    res
                };

                let mut ids = search_results[0].keys().cloned().collect::<HashSet<Id>>();
                for r in search_results.iter().skip(1) {
                    ids = ids.into_iter().filter(|k| r.contains_key(k)).collect();
                }

                ids.iter()
                    .filter_map(|k| {
                        let eclass = *k;
                        let mut inner_results = search_results
                            .iter_mut()
                            .map(|m| m.remove(k).unwrap())
                            .collect::<Vec<SearchMatches>>();
                        // Take all search results and foreach pattern find the common variables and split by them.
                        let by_vars = Searcher::group_by_common_vars(
                            inner_results.iter_mut().collect(),
                            &self.common_vars,
                        );
                        let initial_limits = (0..self.common_vars.len()).map(|_| &None).collect();
                        let res =
                            Searcher::aggregate_substs(&by_vars[..], initial_limits, &self.vars());
                        if res.is_empty() {
                            None
                        } else {
                            Some(SearchMatches {
                                substs: res,
                                eclass,
                            })
                        }
                    })
                    .collect()
            }
        }
        impl HasVars for MultiEqSearcher {
            fn vars(&self) -> Vec<Var> {
                Vec::from_iter(self.patterns.iter().flat_map(|p| p.vars()))
            }
        }
        impl Clone for MultiEqSearcher {
            fn clone(&self) -> Self {
                MultiEqSearcher::new(self.patterns.clone())
            }
        }
        impl Debug for MultiEqSearcher {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(&self.patterns).finish()
            }
        }
        impl PrettyString for MultiEqSearcher {
            fn pretty_string(&self) -> String {
                self.patterns
                    .iter()
                    .map(|p| p.pretty_string())
                    .intersperse(" ||| ".to_string())
                    .collect()
            }
        }
    }
    pub mod multidiff {
        use super::*;
        pub struct MultiDiffSearcher {
            patterns: Vec<Searcher>,
            common_vars: HashMap<Var, usize>,
        }
        impl MultiDiffSearcher {
            pub fn new(patterns: Vec<impl AsSearcher>) -> MultiDiffSearcher {
                let mut patterns: Vec<Searcher> =
                    patterns.into_iter().map(|p| p.as_searcher()).collect();
                let common_vars = Searcher::get_common_vars(&mut patterns);
                MultiDiffSearcher {
                    patterns,
                    common_vars,
                }
            }
        }
        impl FromStr for MultiDiffSearcher {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let patterns = s
                    .split("||||")
                    .map(|p| p.as_searcher())
                    .collect::<Vec<Searcher>>();
                if patterns.len() == 1 {
                    Err(String::from("Need at least two patterns"))
                } else {
                    Ok(MultiDiffSearcher::new(patterns))
                }
            }
        }
        impl AsSearcher for MultiDiffSearcher {
            fn as_searcher(self) -> Searcher {
                Searcher::MultiDiff(self)
            }
        }
        impl HasSearchEClass for MultiDiffSearcher {
            fn search_eclass<G: EGraphView>(
                &self,
                egraph: &G,
                eclass: Id,
            ) -> Option<SearchMatches> {
                unimplemented!()
            }
        }
        impl HasSearch for MultiDiffSearcher {
            fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                if self.patterns.len() == 1 {
                    return self.patterns[0].search(egraph);
                }

                // TODO: we dont need a hashmap here
                let search_results = {
                    let mut res = Vec::new();
                    for p in self.patterns.iter() {
                        let mut current = HashMap::new();
                        let searched = p.search(egraph);
                        for s in searched {
                            assert!(!current.contains_key(&s.eclass));
                            current.insert(s.eclass, s);
                        }
                        res.push(current);
                    }
                    res
                };

                // To reuse group_by_common_vars we will merge all results to a single searchmatches.
                // We don't really care which eclass we use so we will choose the first one.
                // It is a really stupid way to do it but we will run the grouping for each eclass from
                // the first one.
                let mut it = search_results.into_iter();
                let first = it.next();
                // I want to merge all subst except from first
                let mut all_matches = it
                    .map(|mut m| {
                        m.into_iter()
                            .map(|x| x.1)
                            .fold1(|mut s1, mut s2| {
                                s1.substs.extend(s2.substs.into_iter());
                                s1
                            })
                            .unwrap_or(SearchMatches {
                                substs: Vec::new(),
                                eclass: Id::default(),
                            })
                    })
                    .collect::<Vec<SearchMatches>>();
                if all_matches.iter().any(|s| s.substs.is_empty()) {
                    return Vec::new();
                }

                let mut all_combinations = Searcher::group_by_common_vars(
                    all_matches.iter_mut().collect(),
                    &self.common_vars,
                );
                first
                    .unwrap()
                    .into_iter()
                    .filter_map(|(k, mut matches)| {
                        let eclass = k;
                        let first_grouped =
                            Searcher::group_by_common_vars(vec![&mut matches], &self.common_vars)
                                .pop()
                                .unwrap();
                        all_combinations.push(first_grouped);
                        let initial_limits = (0..self.common_vars.len()).map(|_| &None).collect();
                        let res = Searcher::aggregate_substs(
                            &all_combinations[..],
                            initial_limits,
                            &self.vars(),
                        );
                        all_combinations.pop();
                        if res.is_empty() {
                            None
                        } else {
                            Some(SearchMatches {
                                substs: res,
                                eclass,
                            })
                        }
                    })
                    .collect()
            }
        }
        impl HasVars for MultiDiffSearcher {
            fn vars(&self) -> Vec<Var> {
                Vec::from_iter(self.patterns.iter().flat_map(|p| p.vars()).sorted().dedup())
            }
        }
        impl Clone for MultiDiffSearcher {
            fn clone(&self) -> Self {
                MultiDiffSearcher::new(self.patterns.clone())
            }
        }
        impl Debug for MultiDiffSearcher {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(&self.patterns).finish()
            }
        }
        impl PrettyString for MultiDiffSearcher {
            fn pretty_string(&self) -> String {
                self.patterns
                    .iter()
                    .map(|p| p.pretty_string())
                    .intersperse(" ||| ".to_string())
                    .collect()
            }
        }
    }
    pub mod filter {
        use std::sync::Arc;

        use crate::adapter::Searchable;

        use super::*;

        pub type Filter =
            Arc<dyn Fn(&dyn Searchable, Vec<SearchMatches>) -> Vec<SearchMatches> + Send + Sync>;

        #[derive(Clone)]
        pub struct FilterSearcher {
            searcher: Arc<Searcher>,
            filter: Filter,
        }

        impl FilterSearcher {
            pub fn new(searcher: impl AsSearcher, filter: Filter) -> Self {
                FilterSearcher {
                    searcher: Arc::new(searcher.as_searcher()),
                    filter,
                }
            }
            pub fn combine_filters(fs: Vec<Filter>) -> Filter {
                Arc::new(move |searchable, mut original| {
                    for f in fs.iter() {
                        original = f(searchable, original)
                    }
                    original
                })
            }
            pub fn exist_filter(searcher: impl AsSearcher, root: Var, existence: bool) -> Filter {
                let searcher = searcher.as_searcher();
                Arc::new(move |searchable, original| {
                    let requirements = searchable
                        .search(&searcher)
                        .iter()
                        .map(|s| s.eclass)
                        .collect::<HashSet<Id>>();
                    original
                        .into_iter()
                        .filter_map(|mut sm| {
                            let substs = std::mem::take(&mut sm.substs);
                            sm.substs = substs
                                .into_iter()
                                .filter(|s| requirements.contains(&s[root]) == existence)
                                .collect_vec();
                            if sm.substs.is_empty() {
                                None
                            } else {
                                Some(sm)
                            }
                        })
                        .collect_vec()
                })
            }
        }
        impl AsSearcher for FilterSearcher {
            fn as_searcher(self) -> Searcher {
                Searcher::Filter(self)
            }
        }
        impl HasSearchEClass for FilterSearcher {
            fn search_eclass<G: EGraphView>(
                &self,
                egraph: &G,
                eclass: Id,
            ) -> Option<SearchMatches> {
                unimplemented!()
            }
        }
        impl HasSearch for FilterSearcher {
            fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                let matches = self.searcher.search(egraph);
                self.filter.deref()(egraph, matches)
            }
        }
        impl HasVars for FilterSearcher {
            fn vars(&self) -> Vec<Var> {
                self.searcher.vars()
            }
        }
        impl Debug for FilterSearcher {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(&self.searcher, f)
            }
        }
        impl PrettyString for FilterSearcher {
            fn pretty_string(&self) -> String {
                self.searcher.pretty_string()
            }
        }
    }
    pub mod pointer {

        use std::sync::Arc;

        use super::*;
        #[derive(Clone, Debug)]
        pub struct PointerSearcher {
            searcher: Arc<Searcher>,
        }
        impl PointerSearcher {
            pub fn new(searcher: impl AsSearcher) -> Self {
                PointerSearcher {
                    searcher: Arc::new(searcher.as_searcher()),
                }
            }
        }
        impl AsSearcher for PointerSearcher {
            fn as_searcher(self) -> Searcher {
                Searcher::Pointer(self)
            }
        }
        impl HasSearchEClass for PointerSearcher {
            fn search_eclass<G: EGraphView>(
                &self,
                egraph: &G,
                eclass: Id,
            ) -> Option<SearchMatches> {
                self.searcher.search_eclass(egraph, eclass)
            }
        }
        impl HasSearch for PointerSearcher {
            fn search<G: EGraphView>(&self, egraph: &G) -> Vec<SearchMatches> {
                self.searcher.search(egraph)
            }
        }
        impl HasVars for PointerSearcher {
            fn vars(&self) -> Vec<Var> {
                self.searcher.vars()
            }
        }
        impl PrettyString for PointerSearcher {
            fn pretty_string(&self) -> String {
                self.searcher.pretty_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::egg::{RecExpr, SymbolLang};
    use crate::{
        adapter::{EGraph, EasterEgg, Veg, VegCloningBasic, VegCloningPersistent},
        eggstentions::searchers::multisearcher::*,
    };

    fn eq_two_trees_one_common<G: EGraph>() {
        let searcher = multieq::MultiEqSearcher::from_str("(a ?b ?c) ||| (a ?c ?d)").unwrap();
        let mut egraph: G = G::default();
        let x = egraph.add_expr(&RecExpr::from_str("x").unwrap());
        let z = egraph.add_expr(&RecExpr::from_str("z").unwrap());
        let a = egraph.add_expr(&RecExpr::from_str("(a x y)").unwrap());
        egraph.add_expr(&RecExpr::from_str("(a z x)").unwrap());
        egraph.rebuild();
        assert_eq!(searcher.search(&egraph).len(), 0);
        let a2 = egraph.add(SymbolLang::new("a", vec![z, x]));
        egraph.union(a, a2);
        egraph.rebuild();
        assert_eq!(searcher.as_searcher().search(&egraph).len(), 1);
    }

    fn diff_two_trees_one_common<G: EGraph>() {
        let searcher = multidiff::MultiDiffSearcher::from_str("(a ?b ?c) |||| (a ?c ?d)").unwrap();
        let mut egraph: G = G::default();
        let x = egraph.add_expr(&RecExpr::from_str("x").unwrap());
        let z = egraph.add_expr(&RecExpr::from_str("z").unwrap());
        let a = egraph.add_expr(&RecExpr::from_str("(a x y)").unwrap());
        egraph.add_expr(&RecExpr::from_str("(a z x)").unwrap());
        egraph.rebuild();
        assert_eq!(searcher.search(&egraph).len(), 1);
    }

    fn find_ind_hyp<G: EGraph>() {
        let mut egraph: G = G::default();
        let full_pl = egraph.add_expr(&"(pl (S p0) Z)".parse().unwrap());
        let after_pl = egraph.add_expr(&"(S (pl p0 Z))".parse().unwrap());
        let sp0 = egraph.add_expr(&"(S p0)".parse().unwrap());
        let ind_var = egraph.add_expr(&"ind_var".parse().unwrap());
        egraph.union(ind_var, sp0);
        let ltwf = egraph.add_expr(&"(ltwf p0 (S p0))".parse().unwrap());
        egraph.union(full_pl, after_pl);
        egraph.rebuild();
        let searcher =
            multidiff::MultiDiffSearcher::from_str("(ltwf ?x ind_var) |||| (pl ?x Z)").unwrap();
        assert!(!searcher.search(&egraph).is_empty());
    }

    macro_rules! test_impl {
        ($module: ident, $implementation: tt) => {
            #[cfg(test)]
            mod $module {
                use super::*;
                type Impl = $implementation;
                #[test]
                fn eq_two_trees_one_common() {
                    super::eq_two_trees_one_common::<Impl>()
                }
                #[test]
                fn diff_two_trees_one_common() {
                    super::diff_two_trees_one_common::<Impl>()
                }
                #[test]
                fn find_ind_hyp() {
                    super::find_ind_hyp::<Impl>()
                }
            }
        };
    }

    test_impl!(easter_egg, EasterEgg);
    test_impl!(veg, Veg);
    test_impl!(vegcloning, VegCloningBasic);
    test_impl!(vegcloning_persistent, VegCloningPersistent);
}
