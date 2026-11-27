# ChartFrame

Wraps a Chart.js chart, such as files by extension or by size.

**Behaviour:** the chart is built from the current theme's token values (a canvas cannot read CSS variables) and rebuilt when the theme changes, including when the OS switches it; it is destroyed when the view closes. Only the bar chart parts of Chart.js are registered: bars are the default, not a rule. Another chart type (doughnut, line, …) is welcome where it shows something better; adding it means registering its controller here and describing it on this page.

**Rules for charts:** sort bars by value, label categories directly, use the category and status colors, and never let color alone carry meaning.

**Accessibility:** the chart is an image named by its label; the same data is always provided as a table, read by assistive technology and shown on request.
