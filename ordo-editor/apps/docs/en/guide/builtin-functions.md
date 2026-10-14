# Built-in Functions

Every function below can be used in any expression. Try one with `ordo eval 'len("hello")'`.

Functions are strict about types. `upper(null)` and `abs("-5")` are errors, not empty values. Check optional fields with `exists(...)` or `coalesce(...)` first.

## Strings

| Function | Example | Result |
| --- | --- | --- |
| `len(s)` | `len("hello")` | `5` |
| `upper(s)`, `lower(s)` | `upper("vip")` | `"VIP"` |
| `trim(s)` | `trim("  a ")` | `"a"` |
| `starts_with(s, prefix)` | `starts_with("ORD-1", "ORD-")` | `true` |
| `ends_with(s, suffix)` | `ends_with("a.pdf", ".pdf")` | `true` |
| `contains_str(s, sub)` | `contains_str("abc", "b")` | `true` (same as `"abc" contains "b"`) |
| `substring(s, start, end?)` | `substring("hello", 1, 3)` | `"el"` |
| `index_of(s, sub)` | `index_of("abc", "c")` | `2`, or `-1` |
| `char_at(s, i)` | `char_at("abc", 1)` | `"b"` |
| `replace(s, from, to)` | `replace("a-b", "-", "_")` | `"a_b"` |
| `split(s, sep)` | `split("a,b", ",")` | `["a", "b"]` |
| `join(arr, sep)` | `join(["a", "b"], "-")` | `"a-b"` |
| `pad_left(s, len, ch)`, `pad_right(...)` | `pad_left("7", 3, "0")` | `"007"` |
| `format(template, ...)` | `format("{0}-{1}", 1, 2)` | `"1-2"` |
| `sprintf(template, arr)` | `sprintf("%s-%d", ["a", 1])` | `"a-1"` |

## Regular expressions

The pattern comes first.

| Function | Example | Result |
| --- | --- | --- |
| `regex_match(pattern, s)` | `regex_match("^A[0-9]+$", "A12")` | `true` |
| `regex_find(pattern, s)` | `regex_find("[0-9]+", "a12")` | `"12"` |
| `regex_find_all(pattern, s)` | `regex_find_all("[0-9]", "a1b2")` | `["1", "2"]` |
| `regex_replace(pattern, s, repl)` | `regex_replace("[0-9]", "a1b2", "#")` | `"a#b#"` |
| `regex_split(pattern, s)` | `regex_split(",", "a,b")` | `["a", "b"]` |
| `glob_match(pattern, s)` | `glob_match("*.rs", "main.rs")` | `true`. `pattern` may be an array |

## Numbers

| Function | Example | Result |
| --- | --- | --- |
| `abs(n)` | `abs(-5)` | `5` |
| `floor(n)`, `ceil(n)` | `floor(-3.1)` | `-4` |
| `round(n, digits?)` | `round(19.99 * 0.85, 2)` | `16.99` |
| `min(a, b, ...)`, `max(a, b, ...)` | `max(3, 1, 2)` | `3` |
| `decimal(x)` | `decimal("19.99") * 3` | `59.97`, exact |

`round` rounds half away from zero (`round(-3.5)` is `-4`). `min` and `max` take separate values, not an array. For the largest item of an array, use `last(sort(arr))`.

## Arrays

| Function | Example | Result |
| --- | --- | --- |
| `len(arr)`, `count(arr)` | `count([1, 2])` | `2` |
| `sum(arr)`, `avg(arr)` | `sum([1, 2, 3])` | `6`. Both are `0` for `[]` |
| `first(arr)`, `last(arr)` | `first([1, 2])` | `1`, or `null` |
| `sort(arr)`, `reverse(arr)`, `unique(arr)` | `unique([1, 1, 2])` | `[1, 2]` |
| `slice(arr, start, end)` | `slice([1, 2, 3], 0, 2)` | `[1, 2]` |
| `flatten(arr)` | `flatten([[1], [2]])` | `[1, 2]` |
| `array_concat(a, b)` | `array_concat([1], [2])` | `[1, 2]` |
| `range(start, end)` | `range(0, 3)` | `[0, 1, 2]` |
| `map_extract(arr, key)` | `map_extract(items, "price")` | the `price` of each item |
| `filter_by(arr, key, value)` | `filter_by(items, "type", "a")` | items whose `type` is `"a"` |
| `group_by(arr, key)` | `group_by(items, "type")` | an object of arrays |

To add up a field of a list: `sum(map_extract(order.items, "price"))`.

## Objects

| Function | Example | Result |
| --- | --- | --- |
| `keys(obj)`, `values(obj)` | `keys({"a": 1})` | `["a"]` |
| `has_key(obj, key)` | `has_key(user, "email")` | `true` |
| `object_get(obj, key, default)` | `object_get(limits, "eu", 0)` | the value, or `0` |
| `merge(a, b)` | `merge({"a": 1}, {"b": 2})` | `{"a": 1, "b": 2}` |

## Types and missing values

| Function | Notes |
| --- | --- |
| `exists(field)` | true if the field is present and not null |
| `coalesce(a, b, ...)` | first value that is not null |
| `type(x)` | `"int"`, `"float"`, `"string"`, `"array"`, `"decimal"` … |
| `is_null`, `is_number`, `is_string`, `is_bool`, `is_array`, `is_object` | type checks |
| `to_int`, `to_float`, `to_string`, `to_bool` | conversions, e.g. `to_int("42")` |

## Time

Times are Unix timestamps in seconds (UTC).

| Function | Example | Result |
| --- | --- | --- |
| `now()`, `now_millis()` | `now()` | current time |
| `parse_time(s, fmt)` | `parse_time("2024-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")` | `1704067200` |
| `format_time(ts, fmt)` | `format_time(0, "%Y")` | `"1970"` |
| `date_add(ts, n, unit)` | `date_add(0, 1, "days")` | `86400` |
| `time_diff(t1, t2, unit)` | `time_diff(t1, t2, "hours")` | `t1 - t2` in hours |
| `day_of_week(ts)` | `day_of_week(0)` | `4` (Monday is 1, Sunday is 7) |
| `time_of_day(ts)` | `time_of_day(3600)` | `"01:00:00"` |

Units are `seconds`, `minutes`, `hours`, `days` (or `s`, `m`, `h`, `d`).

## Encoding, networks, versions

| Function | Example | Result |
| --- | --- | --- |
| `json_marshal(x)`, `json_unmarshal(s)` | `json_unmarshal("{\"a\":1}")` | `{"a": 1}` |
| `yaml_marshal(x)`, `yaml_unmarshal(s)` | | |
| `url_encode(s)`, `url_decode(s)` | `url_encode("a b")` | `"a%20b"` |
| `hex_encode(s)`, `hex_decode(s)` | `hex_encode("a")` | `"61"` |
| `jwt_decode(token)` | | the payload, without checking the signature |
| `jwt_verify(token, secret, alg)` | | HMAC (`HS256`, `HS384`, `HS512`) |
| `cidr_contains(cidr, ip)` | `cidr_contains("10.0.0.0/8", "10.1.2.3")` | `true` |
| `cidr_intersects(a, b)` | `cidr_intersects("10.0.0.0/8", "10.1.0.0/16")` | `true` |
| `semver_compare(a, b)` | `semver_compare("1.2.0", "1.10.0")` | `-1` |
| `semver_is_valid(s)` | `semver_is_valid("1.2.3")` | `true` |
