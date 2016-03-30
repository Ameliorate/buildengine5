be = require("buildengine")
test_val = false
be.subscribe("test", function ()
    test_val = true
end)
