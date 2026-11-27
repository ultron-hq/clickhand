# dmgbuild settings for the Clickhand disk image: the app on the left, Applications on the
# right, over a background that says "Drag Clickhand into Applications". dmgbuild writes the
# Finder layout itself, so this works on CI machines where scripting Finder doesn't.
import os.path

app = defines["app"]  # noqa: F821  (dmgbuild passes -D values as `defines`)
files = [app]
symlinks = {"Applications": "/Applications"}
background = defines["background"]  # noqa: F821
default_view = "icon-view"
window_rect = ((200, 120), (660, 400))
icon_size = 112
text_size = 13
icon_locations = {os.path.basename(app): (180, 220), "Applications": (480, 220)}
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
