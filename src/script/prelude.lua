prelude_buildengine = {}

function prelude_buildengine.package_searcher (modname)
    -- Assumes that prelude_buildengine.modules has the source code of all the modules that can be imported.
    -- That table is added in a step of initing the interpreter.
    modsrc = prelude_buildengine.modules[modname]
    if modsrc == nil then
        return nil
    end
    return load(modsrc, modname)
end

table.insert(package.searchers, 1, prelude_buildengine.package_searcher)

function prelude_buildengine.call_prelude_fn ()
    -- Assumes that prelude_buildengine.fn_to_call is a string pointing to the function to call, and
    -- prelude_buildengine.args is the arguments to that function. The return value of the function is then placed in
    -- prelude_buildengine.ret.
    -- This function is used to call functions with arguments in rust,
    -- since it isn't exposed in hlua.
    if next(prelude_buildengine.args) ~= nil then
        local ret = prelude_buildengine[prelude_buildengine.fn_to_call](unpack(prelude_buildengine.args))
    else
        local ret = prelude_buildengine[prelude_buildengine.fn_to_call]()
    end
    prelude_buildengine.ret = ret
end
