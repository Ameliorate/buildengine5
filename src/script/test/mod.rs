use std::collections::HashMap;

use super::*;

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
    scripts.insert("init".to_owned(), REQUIRE.to_owned());    // Perhaps rustfmt could do a little better here?
    Engine::new(scripts);
}
