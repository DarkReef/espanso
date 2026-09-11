mod studio_impl {
    include!("app_legacy.rs");
    include!("app_shell.rs");
}

pub use studio_impl::run_shell as run;
