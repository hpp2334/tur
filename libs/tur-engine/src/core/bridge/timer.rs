use boa_engine::Context;

pub fn flush_timers(context: &mut Context) {
    let source = r#"
(function() {
  var maxIterations = 100;
  for (var iter = 0; iter < maxIterations; iter++) {
    var q = globalThis.__timer_queue;
    if (!q || q.length === 0) break;
    var callbacks = q.splice(0, q.length);
    for (var i = 0; i < callbacks.length; i++) {
      if (typeof callbacks[i] === "function") {
        try { callbacks[i](); } catch(e) {}
      }
    }
  }
})();
"#;
    let _ = context.eval(boa_engine::Source::from_bytes(source));
}

pub fn register_timer_globals(context: &mut Context) {
    let source = r#"
globalThis.__timer_queue = [];
globalThis.__timer_call_count = 0;
globalThis.console = {
    log: function() {},
    warn: function() {},
    error: function() {},
    info: function() {},
    debug: function() {},
};
globalThis.setTimeout = function(fn, delay) {
  globalThis.__timer_queue.push(fn);
  globalThis.__timer_call_count++;
  return 1;
};
globalThis.setInterval = function(fn, delay) {
  globalThis.__timer_queue.push(fn);
  globalThis.__timer_call_count++;
  return 1;
};
globalThis.clearTimeout = function(id) {};
globalThis.clearInterval = function(id) {};
"#;
    context
        .eval(boa_engine::Source::from_bytes(source))
        .expect("failed to register timer globals");
}
