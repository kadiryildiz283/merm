use rquickjs::{Context, Runtime};

#[test]
fn test_rquickjs_runtime_eval() {
    let runtime = Runtime::new().expect("Failed to create QuickJS runtime");
    let context = Context::full(&runtime).expect("Failed to create QuickJS context");

    let result: String = context.with(|ctx| {
        let val: String = ctx.eval("`merm-quickjs-${2000 + 26}`").expect("Eval failed");
        val
    });

    assert_eq!(result, "merm-quickjs-2026");
}
