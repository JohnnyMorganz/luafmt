Promise.new():andThen(callThis):andThen(function() end):andThen()

Promise.new():andThen(callThis):andThen({
    true
  }):andThen()

this.is.a.large.start:andThen():andThen(function()
end):andThen()

local f = this:andThen(callThis):andThen({
	true
}).X.Y.Z

this:andThen(callThis):andThen({
	true
}).X.Y.Z:andThen():andThen()

function foo()
	Promise.new():andThen(callThis):andThen(function() end):andThen()
end

local x = {
	promise = Promise.new():andThen(callThis):andThen(function() end):andThen()
}