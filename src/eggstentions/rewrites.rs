use crate::eggstentions::{appliers::{Applier, AsApplier}, searchers::multisearcher::{AsSearcher, Searcher, HasVars}};

#[derive(Clone, Debug)]
pub struct Rewrite {
    name: String,
    long_name: String,
    pub searcher: Searcher,
    pub applier: Applier,
}

impl Rewrite {
    pub fn new<N: Into<String>, S: AsSearcher, A: AsApplier>(
        name: N, 
        searcher: S, 
        applier: A
    ) -> Result<Rewrite, String> {
        let name: String = name.into();
        let long_name: String = name.clone();
        let searcher: Searcher = searcher.as_searcher();
        let applier: Applier = applier.as_applier();

        let bound_vars = searcher.vars();
        for v in applier.vars() {
            if !bound_vars.contains(&v) {
                return Err(format!("Rewrite {} refers to unbound var {}", name, v));
            }
        }

        Ok(Self {
            name,
            long_name,
            searcher,
            applier,
        })
    }
    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn long_name(&self) -> &String {
        &self.long_name
    }
}

#[macro_export]
macro_rules! rewrite {
    ($name: expr; $lhs: expr => $rhs: expr) => {{
        let name: String = ($name).into();
        $crate::eggstentions::rewrites::Rewrite::new(name, $lhs, $rhs).unwrap()
    }};
}
pub use rewrite;