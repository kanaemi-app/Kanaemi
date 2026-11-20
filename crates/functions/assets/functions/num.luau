-- 数を、引数の書き方のそれぞれで書く（引数がなければ half wide kanji。12 → 12・１２・十二）
-- num: a number in each way the argument names, apart by spaces (half wide
-- kanji without one: 12 → 12, １２, 十二). Each value carries its way as the
-- argument that gives it alone, so a way picked holds for every number.
local number = require("./lib/number")

local WAYS = {
	half = function(digits)
		return number.each(digits, number.ASCII)
	end,
	wide = function(digits)
		return number.each(digits, number.WIDE)
	end,
	["kanji-digits"] = function(digits)
		return number.each(digits, number.KANJI)
	end,
	kanji = function(digits)
		return kanaemi.number.counted(digits, false)
	end,
	daiji = function(digits)
		return kanaemi.number.counted(digits, true)
	end,
	grouped = number.grouped,
}

return function(source, argument)
	local digits = kanaemi.number.digits(source)
	if not digits then
		return nil
	end
	local values = {}
	for way in (argument or "half wide kanji"):gmatch("%S+") do
		local write = WAYS[way]
		if not write then
			return nil
		end
		local text = write(digits)
		if text then
			table.insert(values, { text = text, argument = way })
		end
	end
	return values
end
