//! Contatins code relating to the engine's scripting capability.

#[cfg(test)]
mod test;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fmt;

use hlua::{Lua, LuaError, LuaFunction, LuaTable};
use hlua::any::AnyLuaValue;

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
    /// The prelude_buildengine.modules table is initalized with the source code of the scripts passed through the scripts parameter,
    /// sans the init entry, which is executed.
    pub fn new(mut scripts: HashMap<String, String>) -> Result<Self, LuaError> {
        scripts.insert("buildengine".to_owned(), ENGINE_STD.to_owned());
        let mut lua = Lua::new();
        lua.openlibs();
        lua.execute::<()>(PRELUDE).expect("Error in prelude module of engine");
        let mut main = "".to_owned();
        {
            // Set up module table.
            let mut prelude_table: LuaTable<_> = lua.get("prelude_buildengine")
                                                    .expect("Loaded prelude but \
                                                             prelude_buildengine table was not \
                                                             found");
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
        try!(lua.execute::<()>(&main));
        Ok(Engine { interpreter: lua })
    }

    /// Call a given lua event with the given arguments.
    ///
    /// This calls every event with the name, with first the arguments vector passed, then the return of the last event,
    /// then the return of that event, and so on, untill all events of the name have been called.
    /// The returns of that event is then returned.
    pub fn exec_event(&mut self,
                      event_name: String,
                      mut args: Vec<AnyLuaValue>)
                      -> Result<Vec<AnyLuaValue>, ExecEventError> {
        args.insert(0, AnyLuaValue::LuaString(event_name));
        {
            let mut prelude_table: LuaTable<_> = self.interpreter
                                                     .get("prelude_buildengine")
                                                     .expect("The prelude_table wasn't found. \
                                                              Was the prelude properly loaded?");
            let a_event: Option<_> = prelude_table.get::<LuaFunction<_>, _>("activate_event");
            if a_event.is_none() {
                return Err(ExecEventError::EngineStdNotImported);
            }
        }
        match self.call_prelude_fn("activate_event", args) {
            Ok(Some(ret)) => Ok(any_lua_to_vec(ret)),
            Ok(None) => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
    }

    /// Call the given lua function in the prelude table with the given arguments.
    pub fn call_prelude_fn(&mut self,
                           fn_to_call: &str,
                           args: Vec<AnyLuaValue>)
                           -> Result<Option<AnyLuaValue>, LuaError> {
        let mut prelude_table: LuaTable<_> = self.interpreter
                                                 .get("prelude_buildengine")
                                                 .expect("The prelude_table wasn't found. Was \
                                                          the prelude properly loaded?");
        prelude_table.set("fn_to_call", fn_to_call);
        prelude_table.set("args", args);
        {
            let mut call_fn_lua: LuaFunction<_> = prelude_table.get("call_prelude_fn")
                                                               .expect("prelude_buildengine.\
                                                                        call_prelude_fn not \
                                                                        found. Was the prelude \
                                                                        properly loaded?");
            try!(call_fn_lua.call::<()>());
        }
        let ret: Option<AnyLuaValue> = prelude_table.get("ret");
        Ok(ret)
    }
}

impl<'lua> Debug for Engine<'lua> {
    fn fmt(&self, _fmt: &mut Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

/// An error that can ocour executing an event.
#[derive(Debug)]
pub enum ExecEventError {
    /// The buildengine standard library was not imported, so events aren't avalable.
    EngineStdNotImported,
    /// A lua error ocoured executing the event.
    LuaError(LuaError),
}

impl Display for ExecEventError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            ExecEventError::EngineStdNotImported => {
                write!(fmt,
                       "The standard library for the engine was not imported while trying to \
                        execute an event")
            }
            ExecEventError::LuaError(ref _err) => {
                write!(fmt,
                       "An unknown lua error occoured while executing an event.")
            }
        }
    }
}

impl Error for ExecEventError {
    fn description(&self) -> &str {
        match *self {
            ExecEventError::EngineStdNotImported => {
                "The standard library for the engine was not imported while trying to execute an \
                 event"
            }
            ExecEventError::LuaError(ref _err) => {
                "An unknown lua error occoured while executing an event."
            }
        }
    }
}

impl From<LuaError> for ExecEventError {
    fn from(err: LuaError) -> Self {
        ExecEventError::LuaError(err)
    }
}

/// Converts a lua array with whole, numeric keys to a rust vector.
pub fn any_lua_to_vec(any: AnyLuaValue) -> Vec<AnyLuaValue> {
    let as_array = match any {
        AnyLuaValue::LuaArray(arr) => arr, // Ye a pirate!
        AnyLuaValue::LuaOther => return Vec::new(), // Basically only nil passes through here.
        _ => panic!("Called any_lua_to_vec on a non-array lua value: {:?}", any),
    };
    let mut vec: Vec<AnyLuaValue> = Vec::new();
    for value in as_array {
        let (index, value) = value;
        let index = match index {
            AnyLuaValue::LuaNumber(num) => num,
            _ => panic!("Called any_lua_to_vec on array with non-number indexes"),
        };
        let index = index as usize;
        vec.insert(index, value);
    }
    vec
}
