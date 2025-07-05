pub mod debug {
    use std::sync::atomic::AtomicUsize;

    pub const DEBUG: bool = false;
    pub static mut INDENT: AtomicUsize = AtomicUsize::new(0);

    #[macro_export]
    macro_rules! probe {
        ($label: expr, $call: expr, $fmt: expr, $x: expr) => {{
            let mut slabel: String = String::new();
            let mut scall: String = String::new();
            if $crate::util::debug::DEBUG {
                slabel = format!($label);
                scall = format!($call);
                let indent = unsafe {
                    $crate::util::debug::INDENT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                };
                println!("{}[{}] call {}", "--".repeat(indent), slabel, scall);
            }
            let result = $x;
            if $crate::util::debug::DEBUG {
                let indent = unsafe {
                    $crate::util::debug::INDENT.load(std::sync::atomic::Ordering::SeqCst)
                };
                println!(
                    "{}[{}] return {:?}",
                    "--".repeat(indent - 1),
                    slabel,
                    ($fmt)(&result)
                );
                unsafe {
                    $crate::util::debug::INDENT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
            result
        }};
        ($label: expr, $call: expr, $x: expr) => {
            $crate::util::debug::probe!($label, $call, |x| format!("{:?}", x), $x)
        };
    }
    pub use probe;

    #[macro_export]
    macro_rules! probe_iterator {
        ($label: expr, $call: expr, $x: expr) => {{
            let text = format!("Iterator({:?})", ($x).join(","));
            $crate::util::debug::probe!($label, $call, |_| text.clone(), $x)
        }};
    }
    pub use probe_iterator;
}

pub mod tools {
    /// Convert the specified reference into a mutable one. It can be used to
    /// modify data structures, in such a way that all views on the data
    /// structure remains unchanged (e.g. update caches). Avoid using this
    /// unless you can't do otherwise.
    ///
    /// Right now, it is only used for rebuilding easteregg before searching,
    /// because sometimes it would complain about searching a dirty egraph.
    pub unsafe fn as_mut<A>(x: &A) -> &mut A {
        #![allow(invalid_reference_casting)]
        #![allow(mut_from_ref)]
        let const_ptr = x as *const A;
        let mut_ptr = const_ptr as *mut A;
        &mut *mut_ptr
    }
}
