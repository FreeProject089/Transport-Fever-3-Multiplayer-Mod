-- Level crossings' types in a room (docs/HOOKS.md, "Level crossing types").
--
-- A crossing's window changes its type natively; the hook reads the click
-- (crates/tpf3mp-hook/src/crossings.rs) as the crossing entity and its new
-- type index, and this names both portably: the crossing by the places of
-- all its nodes, sorted, and the type by its resource file
-- (tpf3mp_proto action::CrossingType). Every game then finds the one
-- crossing whose nodes are exactly there and gives it the type as the window
-- does (tpf3mp/apply.lua, through the hook's carrier), and reads the type
-- back.

local crossings = {}

-- What the player is told when the room does not carry a change.
crossings.REFUSED = "Not in multiplayer yet: changing a level crossing's type"
-- How far, in metres and in any direction, a node may be from its place.
crossings.TOLERANCE = 0.5

local function list(v)
	local out = {}
	if v == nil then return out end
	local ok, n = pcall(function() return #v end)
	if not ok or type(n) ~= "number" then return out end
	for i = 1, n do out[i] = v[i] end
	return out
end

local function rep()
	local r = api.res.railroadCrossingTypeRep
	if r == nil then error("the game has no railroad crossing types", 0) end
	return r
end

-- The RAILROAD_CROSSING component of `entity`, or nil.
function crossings.component(entity)
	if type(entity) ~= "number" or entity < 0 then return nil end
	local ok, c = pcall(api.engine.getComponent, entity, api.type.ComponentType.RAILROAD_CROSSING)
	if ok then return c end
	return nil
end

-- A place's key, in whole millimetres, as the room's wire rounds it.
local function mm(v) return math.floor(v * 1000 + 0.5) end
local function key(p) return { mm(p.x), mm(p.y), mm(p.z) } end
local function before(a, b)
	local ka, kb = key(a), key(b)
	for i = 1, 3 do
		if ka[i] ~= kb[i] then return ka[i] < kb[i] end
	end
	return false
end

-- The places of crossing `c`'s nodes ({ x =, y =, z = } in metres), sorted
-- as the wire wants them (by whole millimetres, x then y then z), and the
-- nodes in the same order. Raises when one does not read or two coincide.
function crossings.places(c)
	local nodes = list(c and c.nodes)
	if #nodes == 0 then error("a crossing with no node", 0) end
	local placed = {}
	for _, node in ipairs(nodes) do
		local ok, b = pcall(api.engine.getComponent, node, api.type.ComponentType.BASE_NODE)
		local p = ok and b and b.position
		if p == nil then error("a crossing node whose place does not read", 0) end
		placed[#placed + 1] = { node = node, at = { x = p.x or p[1], y = p.y or p[2], z = p.z or p[3] } }
	end
	table.sort(placed, function(a, b) return before(a.at, b.at) end)
	local places, ordered = {}, {}
	for i, n in ipairs(placed) do
		if i > 1 and not before(placed[i - 1].at, n.at) then error("two crossing nodes in one place", 0) end
		places[i], ordered[i] = n.at, n.node
	end
	return places, ordered
end

-- The resource file of crossing type `index`, when the game offers it here
-- (visible); raises otherwise.
function crossings.typeName(index)
	local r = rep()
	if type(index) ~= "number" or index < 0 then error("a crossing type that is not one", 0) end
	local ok, name = pcall(r.getName, index)
	if not ok or type(name) ~= "string" or name == "" then error("crossing type " .. tostring(index) .. " has no name", 0) end
	local okVisible, visible = pcall(r.isVisible, index)
	if okVisible and visible == false then error("crossing type " .. name .. " is not offered here", 0) end
	return name
end

-- The index of the crossing type named `name` in this game, when it offers
-- it (visible); raises otherwise.
function crossings.typeIndex(name)
	local r = rep()
	local ok, index = pcall(r.find, name)
	if not ok or type(index) ~= "number" or index < 0 then error("no crossing type " .. tostring(name), 0) end
	local okVisible, visible = pcall(r.isVisible, index)
	if okVisible and visible == false then error("crossing type " .. tostring(name) .. " is not offered here", 0) end
	return index
end

-- The window's change as the hook read it ({ crossingType = true, entity =,
-- type = }) as a SetCrossingType action, or nil and why.
function crossings.capture(native)
	local ok, action = pcall(function()
		local entity, index = native and native.entity, native and native.type
		local c = crossings.component(entity)
		if c == nil then error("the crossing is gone", 0) end
		local name = crossings.typeName(index)
		if c.typeIndex == index then error("the crossing is of that type already", 0) end
		local places = crossings.places(c)
		return { SetCrossingType = { nodes = places, crossing_type = name } }
	end)
	if ok then return action end
	return nil, tostring(action)
end

local function near(a, b)
	local dx, dy, dz = a.x - b.x, a.y - b.y, a.z - b.z
	return dx * dx + dy * dy + dz * dz <= crossings.TOLERANCE * crossings.TOLERANCE
end

-- The crossings with a node within the tolerance of one of `places`: each
-- node on both a street and a track there, by the game's crossing system.
local function candidates(places)
	local streets = api.engine.system.streetSystem
	-- Held while they are read (tpf3mp/apply.lua, readNodes).
	local trackMap, streetMap = streets.getNode2TrackEdgeMap(), streets.getNode2StreetEdgeMap()
	local onTrack = {}
	for node in pairs(trackMap) do onTrack[node] = true end
	local found, seen = {}, {}
	for node in pairs(streetMap) do
		if onTrack[node] then
			local b = api.engine.getComponent(node, api.type.ComponentType.BASE_NODE)
			local p = b and b.position
			if p then
				local at = { x = p.x or p[1], y = p.y or p[2], z = p.z or p[3] }
				for _, place in ipairs(places) do
					if near(at, place) then
						local ok, crossing = pcall(api.engine.system.railRoadCrossingSystem.getRailroadCrossingForNode, node)
						if ok and type(crossing) == "number" and crossing >= 0 and not seen[crossing] then
							seen[crossing] = true
							found[#found + 1] = crossing
						end
						break
					end
				end
			end
		end
	end
	return found
end

-- The one crossing whose nodes are exactly at `places` (sorted, as the
-- action names them): its entity and component. Raises when there is none,
-- or more than one.
function crossings.resolve(places)
	if type(places) ~= "table" or #places == 0 then error("a crossing named by no node", 0) end
	local match
	for _, entity in ipairs(candidates(places)) do
		local c = crossings.component(entity)
		local okPlaces, own = pcall(crossings.places, c)
		if c and okPlaces and #own == #places then
			local same = true
			for i = 1, #own do
				if not near(own[i], places[i]) then same = false break end
			end
			if same then
				if match then error("two crossings where the action names one", 0) end
				match = { entity = entity, comp = c }
			end
		end
	end
	if not match then error("the crossing is not there", 0) end
	return match.entity, match.comp
end

return crossings
