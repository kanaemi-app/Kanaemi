local num = functions["num"]

-- The values num gives, as `text=argument` joined by `|`.
local function given(source, argument)
	local values = num(source, argument)
	if values == nil then
		return nil
	end
	local out = {}
	for _, value in values do
		table.insert(out, `{value.text}={value.argument}`)
	end
	return table.concat(out, "|")
end

test("writes a number half, wide and in kanji without an argument", function()
	eq(given("12"), "12=half|１２=wide|十二=kanji")
	eq(given("１２"), "12=half|１２=wide|十二=kanji")
end)

test("writes a number in the ways the argument names, in its order", function()
	eq(given("12", "kanji daiji"), "十二=kanji|壱拾弐=daiji")
	eq(given("2026", "grouped kanji-digits"), "2,026=grouped|二〇二六=kanji-digits")
	eq(given("3", "wide"), "３=wide")
end)

test("leaves out a way that cannot write the number", function()
	local big = "1" .. string.rep("0", 20)
	eq(given(big, "kanji half"), `{big}=half`)
end)

test("gives nil for an unknown way", function()
	eq(given("12", "half roman"), nil)
end)

test("gives nil for what is no number", function()
	eq(given("きょう"), nil)
end)
