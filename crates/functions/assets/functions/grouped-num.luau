-- 数を 3 けたごとに区切った半角の数字で書く（2026 → 2,026）
-- grouped-num: a number in ASCII digits with a comma between each group of
-- three, without its leading zeros (2026 → 2,026).
local number = require("./lib/number")

return function(source)
	local digits = kanaemi.number.digits(source)
	return digits and number.grouped(digits)
end
