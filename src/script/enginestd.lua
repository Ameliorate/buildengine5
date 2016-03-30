local buildengine = {}
prelude_buildengine.events = {}

function buildengine.subscribe (event_name, action)
    if prelude_buildengine.events[event_name] == nil then
        prelude_buildengine.events[event_name] = { action }
    else
        table.insert(prelude_buildengine.events[event_name], action)
    end
end

function buildengine.activate_event (event_name, ...)
    local event_args = ...
    local events_calling = prelude_buildengine.events[event_name]
    for i,event_calling in pairs(events_calling) do
        if event_args then
            event_args = event_calling(unpack(event_args))
        else
            event_args = event_calling()
        end
    end
    return event_args
end
prelude_buildengine.activate_event = buildengine.activate_event

return buildengine;
