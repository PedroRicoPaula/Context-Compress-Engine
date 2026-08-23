# Shared filter: cargo --message-format=json -> one line per diagnostic.
# Drops ASCII art, code frames, "Compiling x v0.1.0", notes and rendered blocks.
select(.reason == "compiler-message")
| .message as $m
| select($m.level == "error" or $m.level == "warning")
| ($m.spans // [] | map(select(.is_primary)) | .[0]) as $s
| [ ($s | if . == null then "-" else "\(.file_name):\(.line_start):\(.column_start)" end)
  , ($m.level | ascii_upcase)
  , ($m.code.code // "-")
  , ($m.message | gsub("\n"; " "))
  ] | join(" ")
