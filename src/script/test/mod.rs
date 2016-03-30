use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;

use hlua::any::AnyLuaValue;
use hlua::{LuaTable, function0};

use super::*;

const EVENT: &'static str = include_str!("event.lua");
const TEST: &'static str = include_str!("test.lua");
const REQUIRE: &'static str = include_str!("require.lua");

static CALL_FN_NO_ARGS_TEST_VAL: AtomicBool = AtomicBool::new(false);

/// Call Engine.new without any code.
#[test]
fn engine_new_no_code() {
    Engine::new(HashMap::new());
}

/// Tests requiring a module.
#[test]
fn load_module() {
    let mut scripts: HashMap<String, String> = HashMap::new();
    scripts.insert("test".to_owned(), TEST.to_owned());
    scripts.insert("init".to_owned(), REQUIRE.to_owned());
    Engine::new(scripts);
}

/// Tests declaring and raising a lua event.
#[test]
fn lua_event() {
    let mut scripts: HashMap<String, String> = HashMap::new();
    scripts.insert("init".to_owned(), EVENT.to_owned());
    let mut engine = Engine::new(scripts);
    let _ = engine.exec_event("test".to_owned(), Vec::new()).expect("Failed to exec event");
    let test_val: AnyLuaValue = engine.interpreter.get("test_val").unwrap();
    assert_eq!(test_val, AnyLuaValue::LuaBoolean(true));
}

/// Tests calling a prelude function using Engine::call_prelude_fn.
///
/// Curently broken until tomaka/hlua#66
#[test]
fn call_fn_no_args() {
    let mut engine = Engine::new(HashMap::new());
    let fun = function0(|| {
        CALL_FN_NO_ARGS_TEST_VAL.store(true, Ordering::Relaxed);
    });
    {
        let mut prelude_table: LuaTable<_> = engine.interpreter
                                                   .get("prelude_buildengine")
                                                   .expect("Failed to get prelude table.");
        prelude_table.set("test_fn", fun);
    }
    let result = engine.call_prelude_fn("test_fn", Vec::new()).expect("Failed to call test_fn");
    assert!(result.is_none(),
            "Engine::call_prelude_fn returned a Some value for a function returning nil: {:?}",
            result);
}
