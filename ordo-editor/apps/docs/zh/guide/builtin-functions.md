# 内置函数

下面的函数在任何表达式里都能用。可以用 `ordo eval 'len("hello")'` 试一下。

函数对类型要求严格。`upper(null)`、`abs("-5")` 会报错，而不是返回空值。可选字段先用 `exists(...)` 或 `coalesce(...)` 处理。

## 字符串

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `len(s)` | `len("hello")` | `5` |
| `upper(s)`、`lower(s)` | `upper("vip")` | `"VIP"` |
| `trim(s)` | `trim("  a ")` | `"a"` |
| `starts_with(s, prefix)` | `starts_with("ORD-1", "ORD-")` | `true` |
| `ends_with(s, suffix)` | `ends_with("a.pdf", ".pdf")` | `true` |
| `contains_str(s, sub)` | `contains_str("abc", "b")` | `true`（等同于 `"abc" contains "b"`） |
| `substring(s, start, end?)` | `substring("hello", 1, 3)` | `"el"` |
| `index_of(s, sub)` | `index_of("abc", "c")` | `2`，找不到是 `-1` |
| `char_at(s, i)` | `char_at("abc", 1)` | `"b"` |
| `replace(s, from, to)` | `replace("a-b", "-", "_")` | `"a_b"` |
| `split(s, sep)` | `split("a,b", ",")` | `["a", "b"]` |
| `join(arr, sep)` | `join(["a", "b"], "-")` | `"a-b"` |
| `pad_left(s, len, ch)`、`pad_right(...)` | `pad_left("7", 3, "0")` | `"007"` |
| `format(template, ...)` | `format("{0}-{1}", 1, 2)` | `"1-2"` |
| `sprintf(template, arr)` | `sprintf("%s-%d", ["a", 1])` | `"a-1"` |

## 正则

模式写在前面。

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `regex_match(pattern, s)` | `regex_match("^A[0-9]+$", "A12")` | `true` |
| `regex_find(pattern, s)` | `regex_find("[0-9]+", "a12")` | `"12"` |
| `regex_find_all(pattern, s)` | `regex_find_all("[0-9]", "a1b2")` | `["1", "2"]` |
| `regex_replace(pattern, s, repl)` | `regex_replace("[0-9]", "a1b2", "#")` | `"a#b#"` |
| `regex_split(pattern, s)` | `regex_split(",", "a,b")` | `["a", "b"]` |
| `glob_match(pattern, s)` | `glob_match("*.rs", "main.rs")` | `true`。`pattern` 可以是数组 |

## 数字

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `abs(n)` | `abs(-5)` | `5` |
| `floor(n)`、`ceil(n)` | `floor(-3.1)` | `-4` |
| `round(n, digits?)` | `round(19.99 * 0.85, 2)` | `16.99` |
| `min(a, b, ...)`、`max(a, b, ...)`、`max(arr)` | `max(3, 1, 2)` | `3` |
| `decimal(x)` | `decimal("19.99") * 3` | `59.97`，精确 |

`round` 四舍五入时远离零（`round(-3.5)` 是 `-4`）。`min` 和 `max` 接收多个值，或者一个数组：`max(scores)`。

## 数组

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `len(arr)`、`count(arr)` | `count([1, 2])` | `2` |
| `sum(arr)`、`avg(arr)` | `sum([1, 2, 3])` | `6`。对 `[]` 都是 `0` |
| `first(arr)`、`last(arr)` | `first([1, 2])` | `1`，空数组是 `null` |
| `sort(arr)`、`reverse(arr)`、`unique(arr)` | `unique([1, 1, 2])` | `[1, 2]` |
| `slice(arr, start, end)` | `slice([1, 2, 3], 0, 2)` | `[1, 2]` |
| `flatten(arr)` | `flatten([[1], [2]])` | `[1, 2]` |
| `array_concat(a, b)` | `array_concat([1], [2])` | `[1, 2]` |
| `range(start, end)` | `range(0, 3)` | `[0, 1, 2]` |
| `map_extract(arr, key)` | `map_extract(items, "price")` | 每个元素的 `price` |
| `filter_by(arr, key, value)` | `filter_by(items, "type", "a")` | `type` 为 `"a"` 的元素 |
| `group_by(arr, key)` | `group_by(items, "type")` | 按值分组的对象 |

把列表里某个字段加起来：`sum(map_extract(order.items, "price"))`。

## 对象

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `keys(obj)`、`values(obj)` | `keys({"a": 1})` | `["a"]` |
| `has_key(obj, key)` | `has_key(user, "email")` | `true` |
| `object_get(obj, key, default)` | `object_get(limits, "eu", 0)` | 对应的值，没有就是 `0` |
| `merge(a, b)` | `merge({"a": 1}, {"b": 2})` | `{"a": 1, "b": 2}` |

## 类型和缺失值

| 函数 | 说明 |
| --- | --- |
| `exists(field)` | 字段存在且不为 null 时为真 |
| `coalesce(a, b, ...)` | 第一个不为 null 的值 |
| `type(x)` | `"int"`、`"float"`、`"string"`、`"array"`、`"decimal"` 等 |
| `is_null`、`is_number`、`is_string`、`is_bool`、`is_array`、`is_object` | 判断类型 |
| `to_int`、`to_float`、`to_string`、`to_bool` | 类型转换，如 `to_int("42")` |

## 时间

时间是 Unix 时间戳，单位秒（UTC）。

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `now()`、`now_millis()` | `now()` | 当前时间 |
| `parse_time(s, fmt)` | `parse_time("2024-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")` | `1704067200` |
| `format_time(ts, fmt)` | `format_time(0, "%Y")` | `"1970"` |
| `date_add(ts, n, unit)` | `date_add(0, 1, "days")` | `86400` |
| `time_diff(t1, t2, unit)` | `time_diff(t1, t2, "hours")` | `t1 - t2`，单位小时 |
| `day_of_week(ts)` | `day_of_week(0)` | `4`（周一是 1，周日是 7） |
| `time_of_day(ts)` | `time_of_day(3600)` | `"01:00:00"` |

单位可以是 `seconds`、`minutes`、`hours`、`days`（或 `s`、`m`、`h`、`d`）。

## 编码、网络、版本号

| 函数 | 例子 | 结果 |
| --- | --- | --- |
| `json_marshal(x)`、`json_unmarshal(s)` | `json_unmarshal("{\"a\":1}")` | `{"a": 1}` |
| `yaml_marshal(x)`、`yaml_unmarshal(s)` | | |
| `url_encode(s)`、`url_decode(s)` | `url_encode("a b")` | `"a%20b"` |
| `hex_encode(s)`、`hex_decode(s)` | `hex_encode("a")` | `"61"` |
| `jwt_decode(token)` | | 取出载荷，不校验签名 |
| `jwt_verify(token, secret, alg)` | | HMAC（`HS256`、`HS384`、`HS512`） |
| `cidr_contains(cidr, ip)` | `cidr_contains("10.0.0.0/8", "10.1.2.3")` | `true` |
| `cidr_intersects(a, b)` | `cidr_intersects("10.0.0.0/8", "10.1.0.0/16")` | `true` |
| `semver_compare(a, b)` | `semver_compare("1.2.0", "1.10.0")` | `-1` |
| `semver_is_valid(s)` | `semver_is_valid("1.2.3")` | `true` |
