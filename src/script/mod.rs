//! Contatins code relating to the engine's scripting capability.

#[cfg(test)]
mod test;

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::fmt;

use hlua::Lua;
use hlua::lua_tables::LuaTable;

/// The engine lua standard library. Contains functionality relating to making a game with the engine.
///
/// Exposed as the module "buildstation" to lua code.
const ENGINE_STD: &'static str = include_str!("enginestd.lua");

/// A piece of code run before the main script.
const PRELUDE: &'static str = include_str!("prelude.lua");

/// Handles the scripts, their state, and their execution.
pub struct Engine<'lua> {
    /// The interpreter used for the scripts.
    pub interpreter: Lua<'lua>,
}

impl<'lua> Engine<'lua> {
    /// Constructs a script::Engine and loads the given scripts.
    ///
    /// The interpreter is initalized with the lua standard library, and the engine std.
    ///
    /// All scripts are loaded into the same namespace, and must return nothing when loaded.
    pub fn new(mut scripts: HashMap<String, String>) -> Self {
        scripts.insert("buildengine".to_owned(), ENGINE_STD.to_owned());
        let mut lua = Lua::new();
        lua.openlibs();
        lua.execute::<()>(PRELUDE).expect("Syntax error in prelude module of engine");
        let mut main = "".to_owned();
        {
            // Set up module table.
            let mut prelude_table: LuaTable<_> =
                lua.get("prelude_buildengine")
                   .expect("Loaded prelude but prelude_buildengine table was not found");
            let mut modules = prelude_table.empty_array("modules");
            for script in scripts {
                let (name, body) = script;
                if name == "init".to_owned() {
                    main = body;
                    continue;
                }
                modules.set(name, body);
            }
        }
        lua.execute::<()>(&main).expect("Syntax error in init module of script");
        Engine { interpreter: lua }
    }
}

impl<'lua> Debug for Engine<'lua> {
    fn fmt(&self, _fmt: &mut Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}
