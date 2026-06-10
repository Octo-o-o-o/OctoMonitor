# Claude statusline negative fixture: missing session id

This fixture locks a rejected Claude statusline/hook payload shape. A payload that omits
`session_id` must not be promoted to stable monitored operation support and must not produce
a resume command or an approval action.
