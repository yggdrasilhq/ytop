use serde_json::{json, Value};
use std::sync::OnceLock;

pub static YTOP_TRACE_PROVIDER: OnceLock<ytrace::Provider> = OnceLock::new();

pub fn provider() -> &'static ytrace::Provider {
    YTOP_TRACE_PROVIDER.get_or_init(|| {
        let home = ytrace::compat::resolve_home("ytop");
        let p = ytrace::Provider::with_home(
            "ytop",
            env!("CARGO_PKG_VERSION"),
            home,
        );
        // Pre-register probes for ytop self-observation
        for probe in [
            "probe/host_local",
            "probe/host_remote",
            "probe/zfs_iostat",
            "probe/lxc_containers",
            "render/viewport",
            "render/rail",
            "render/notebook_page",
            "action/dispatch",
            "notebook/query",
            "supervision/census",
            "worker/tick",
            "booter/tick",
            "osc/heartbeat",
        ] {
            p.register(probe, ytrace::Clock::Wall, ytrace::Sample::always());
        }
        p
    })
}

pub fn span(category: &'static str, name: &'static str) -> ytrace::SpanGuard<'static> {
    provider().span("core", category, name, json!({}))
}

pub fn span_with(category: &'static str, name: &'static str, ctx: Value) -> ytrace::SpanGuard<'static> {
    provider().span("core", category, name, ctx)
}

pub fn event(category: &'static str, name: &'static str, payload: Value) {
    provider().event("core", category, name, payload);
}
