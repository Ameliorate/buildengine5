prelude_buildengine = {}

function prelude_buildengine.package_searcher (modname)
    -- Assumes that prelude_buildengine.modules has the source code of all the modules that can be imported.
    -- That table is added in a step of initing the interpreter.
    modsrc = prelude_buildengine.modules[modname]
    if modsrc == nil then
        return nil
    end
    return load(modsrc)
end

table.insert(package.searchers, 1, prelude_buildengine.package_searcher)

function prelude_buildengine.call_fn ()
    -- Assumes that prelude_buildengine.fn_to_call is a string pointing to the function to call, and
    -- prelude_buildengine.args is the arguments to that function.
    -- This function is used to call functions with arguments in rust,
    -- since it isn't exposed in hlua.
    _G[prelude_buildengine.fn_to_call](unpack(prelude_buildengine.args))
end
