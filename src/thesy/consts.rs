use crate::egg::Var;
use crate::{adapter::EGraph, eggstentions::appliers};
use crate::eggstentions::searchers::multisearcher::*;
use crate::eggstentions::appliers::*;
use crate::eggstentions::rewrites::{rewrite, Rewrite};
use std::str::FromStr;
use crate::thesy::case_split::{CaseSplit, Split, SplitApplier};
use itertools::Itertools;

pub(crate) fn bool_rws() -> Vec<Rewrite> {
    let and_multi_searcher = multieq::MultiEqSearcher::new(vec!["true", "(and ?x ?y)"]);

    let and_implies = rewrite!("and_implies"; {and_multi_searcher.clone()} => "(= ?x true)");
    let and_implies2 = rewrite!("and_implies2"; {and_multi_searcher} => "(= ?y true)");

    vec![
        rewrite!("or-true"; "(or true ?x)" => "true"),
        rewrite!("or-true2"; "(or ?x true)" => "true"),
        rewrite!("or-false"; "(or false ?x)" => "?x"),
        rewrite!("or-false2"; "(or ?x false)" => "?x"),
        // or_implies,

        rewrite!("and-true"; "(and true ?x)" => "?x"),
        rewrite!("and-true2"; "(and ?x true)" => "?x"),
        rewrite!("and-false"; "(and false ?x)" => "false"),
        rewrite!("and-false2"; "(and ?x false)" => "false"),
        // and_implies,
        // and_implies2,

        rewrite!("not-true"; "(not true)" => "false"),
        rewrite!("not-false"; "(not false)" => "true"),
    ]
}

// Also common that less is skipped
pub(crate) fn less_rws() -> Vec<Rewrite> {
    vec![
        rewrite!("less-zero"; "(less ?x zero)" => "false"),
        rewrite!("less-zs"; "(less zero (succ ?x))" => "true"),
        rewrite!("less-succ"; "(less (succ ?y) (succ ?x))" => "(less ?y ?x)")
    ]
}

fn cons_conc_searcher() -> multieq::MultiEqSearcher {
    multieq::MultiEqSearcher::new(vec!["true", "(is-cons ?x)"])
}

fn cons_conclusion() -> diff::DiffApplier {
    diff::DiffApplier::new("(cons (isconsex ?x))".as_applier())
}

pub(crate) fn is_rws() -> Vec<Rewrite> {
    vec![
        rewrite!("is_cons_true"; 
            {filter::FilterSearcher::new(
                "(is-cons ?x)",
                filter::FilterSearcher::exist_filter("(cons ?y)", Var::from_str("?x").unwrap(), true)
            )}
            => "true"
        ),
        rewrite!("is_cons_false"; 
            {filter::FilterSearcher::new(
                "(is-cons ?x)", 
                filter::FilterSearcher::exist_filter("nil", Var::from_str("?x").unwrap(), true)
            )} 
            => "false"
        ),
        rewrite!("is_cons_conclusion"; 
            {cons_conc_searcher()} 
            => {cons_conclusion()}
        ),
        rewrite!("is_succ_true"; 
            {filter::FilterSearcher::new(
                "(is-succ ?x)", 
                filter::FilterSearcher::exist_filter("(succ ?y)", Var::from_str("?x").unwrap(), true)
            )} 
            => "true"
        ),
        rewrite!("is_succ_false"; 
            {filter::FilterSearcher::new(
                "(is-succ ?x)", 
                filter::FilterSearcher::exist_filter("zero", Var::from_str("?x").unwrap(), true)
            )}
            => "false"
        ),
        rewrite!("is_ESC_true"; 
            {filter::FilterSearcher::new(
                "(is-ESC ?x)", 
                filter::FilterSearcher::exist_filter("ESC", "?x".parse().unwrap(), true)
            )} 
            => "true"
        ),
    ]
}

pub(crate) fn equality_rws() -> Vec<Rewrite> {
    let eq_searcher = multieq::MultiEqSearcher::new(vec!["true", "(= ?x ?y)"]);
    let union_applier = union::UnionApplier::new(vec![Var::from_str("?x").unwrap(), Var::from_str("?y").unwrap()]);
    vec![
        rewrite!("equality"; "(= ?x ?x)" => "true"),
        rewrite!("equality-true"; eq_searcher => union_applier),
        // TODO: I would like to split by equality but not a possibility with current conditions.
        // rewrite!("equality-split"; "(= ?x ?y)" => "(potential_split (= ?x ?y) true false)" if {NonPatternCondition::new(Pattern::from_str("").unwrap(), Var::from_str("?"))})
    ]
}

pub(crate) fn ite_rws() -> Vec<Rewrite> {
    vec![
        rewrite!("ite_true"; "(ite true ?x ?y)" => "?x"),
        rewrite!("ite_false"; "(ite false ?x ?y)" => "?y"),
    ]
}

pub fn system_case_splits<G: EGraph>() -> CaseSplit<G> {
    let ite_searcher = {
        let true_cond = filter::FilterSearcher::exist_filter("true", Var::from_str("?z").unwrap(), false);
        let false_cond = filter::FilterSearcher::exist_filter("false", Var::from_str("?z").unwrap(), false);
        filter::FilterSearcher::new("(ite ?z ?x ?y)", filter::FilterSearcher::combine_filters(vec![true_cond, false_cond])).as_searcher()
    };
    let mut res = CaseSplit::from_applier_patterns(vec![(ite_searcher, Var::from_str("?z").unwrap(), vec!["true".parse().unwrap(), "false".parse().unwrap()])]);

    let or_multi_searcher = multieq::MultiEqSearcher::new(vec!["true", "(or ?x ?y)"]).as_searcher();

    let x_var = Var::from_str("?x").unwrap();
    let y_var = Var::from_str("?y").unwrap();
    let or_implies_applier: SplitApplier<G> = Box::new(move |graph, sms| {
        let true_root = graph.add_expr(&"true".parse().unwrap());
        sms.iter().flat_map(|sm| sm.substs.iter().map(|subs|
            Split::new(true_root, vec![*subs.get(x_var).unwrap(), *subs.get(y_var).unwrap()])
        )).collect_vec()
    });

    res.extend(CaseSplit::new(vec![(or_multi_searcher, or_implies_applier)]));
    res

}