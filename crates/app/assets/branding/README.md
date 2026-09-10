# Terminator branding

The selected logo is **Red Eye**: terminal + AI, with a red machine sensor
as a subtle nod to The Terminator. Generated with the built-in image tool.

`terminator.png` is the original selected artwork from
`output/logo-variants/05-red-eye.png`. The GUI embeds it as its window icon.
`terminator.icns` contains macOS icon sizes resized with `sips` and stored as PNG entries in an ICNS container
and is copied into the application bundle by `cargo xtask package`.

The Linux archive includes the 512 px derivative as `terminator.png`; when installing the included
desktop entry, install this image as `terminator.png` in the user's icon
directory (`~/.local/share/icons/hicolor/512x512/apps/`), or set the desktop
entry's `Icon` value to the installed image's absolute path.
