tear
====

Run commands by swiping in from an edge on a touchscreen.

This program runs in the background and listens to touch events. When a touch "starts" on an edge, and moves away from
the edge, a command is run.

Input is handled using libinput, and is not dependent on X, so it might work on Wayland as well. If your touch events
are displayed by libinput-debug-events, tear should see them as well.

Configuration
-------------

tear looks for configuration in $HOME/.config/tear/tear.toml.

All distances are in percentages, so a distance of 50.0 means half of the screen's size (width or height, depending on
which edge is swiped in from).

```toml
# The swipe must be at least this long to count
distance = 4.0

[[triggers]]
# Commands are executed using sh -c "$command"
command = "influence"

# Valid values are "left", "top", "right", "bottom"
edge    = "bottom"

# Specify a range for this trigger along its edge. If you specify from = 0.0,
# to = 100.0, the trigger will work along the entire edge. from = 90.0,
# to = 100.0 will only work within the last 10% of the edge.
from    = 0.0
to      = 33.3

# Add more triggers using the same format
[[triggers]]
command = "vinyl --popup"
edge    = "bottom"
from    = 33.3
to      = 66.7

[[triggers]]
command = "calendar"
edge    = "bottom"
from    = 66.7
to      = 100.0
```

0% of an edge represents the top-left corner for the left and top edges, the top-right corner for the right edge, and
the bottom-left corner for the bottom edge.
