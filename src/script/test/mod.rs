use std::collections::HashMap;

use hlua::any::AnyLuaValue;

use super::*;

const EVENT: &'static str = include_str!("event.lua");
const TEST: &'static str = include_str!("test.lua");
const REQUIRE: &'static str = include_str!("require.lua");

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
