prelude_buildengine = {}

-- Print contents of `tbl`, with indentation.
-- `indent` sets the initial level of indentation.
function tprint (tbl, indent)
  if not indent then indent = 0 end
  for k, v in pairs(tbl) do
    formatting = string.rep("  ", indent) .. k .. ": "
    if type(v) == "table" then
      print(formatting)
      tprint(v, indent+1)
    elseif type(v) == 'boolean' then
      print(formatting .. tostring(v))
    else
      print(formatting .. v)
    end
  end
end

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
