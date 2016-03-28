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
