//! Contatins code relating to the engine's scripting capability.

use std::fmt::{Debug, Formatter};
use std::fmt;

use hlua::Lua;

const ENGINE_STD: &'static str = include_str!("enginestd.lua");

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
    pub fn new(mut scripts: Vec<&str>) -> Self {
        scripts.push(ENGINE_STD);
        let mut lua = Lua::new();
        lua.openlibs();
        for script in scripts {
            let _ = lua.execute::<()>(script);
        }
        Engine { interpreter: lua }
    }
}

impl<'lua> Debug for Engine<'lua> {
    fn fmt(&self, _fmt: &mut Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}
